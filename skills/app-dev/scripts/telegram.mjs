#!/usr/bin/env node
// Sends to and reads from the team's report chat, as the bot in the project's .env
// (TELEGRAM_BOT_TOKEN, TELEGRAM_CHAT_ID).
//
//   node telegram.mjs check                  the bot, the chat, and whether the bot sees every message
//   node telegram.mjs send "text"            post plain text; with no text, read it from stdin
//   node telegram.mjs send --reply 42 "text" post as a reply to message 42
//   node telegram.mjs read                   messages since the last read, one JSON object per line
//   node telegram.mjs inbox [N]              the last N messages already read (default 20)
//
// The read position is kept in ~/.config/app/<project>-<hash>/telegram-offset and every message
// read is appended to telegram-inbox.jsonl beside it, both mode 600.
// Exit codes: 0 done, 1 Telegram refused, 2 cannot run.
import { createHash } from "node:crypto";
import { appendFileSync, existsSync, mkdirSync, readFileSync, realpathSync, writeFileSync } from "node:fs";
import { homedir } from "node:os";
import { basename, dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const MAX_TEXT = 4096;
const root = realpathSync(resolve(dirname(fileURLToPath(import.meta.url)), "../../.."));
const envFile = join(root, ".env");
// One folder per checkout, so two projects made from the template never share state.
const tag = createHash("sha256").update(root).digest("hex").slice(0, 8);
const dir = join(homedir(), ".config/app", `${basename(root)}-${tag}`);
const offsetFile = join(dir, "telegram-offset");
const inboxFile = join(dir, "telegram-inbox.jsonl");

const env = existsSync(envFile) ? readEnv(readFileSync(envFile, "utf8")) : {};
const token = env.TELEGRAM_BOT_TOKEN ?? "";
const chatId = env.TELEGRAM_CHAT_ID ?? "";

// KEY=value, KEY='value' (literal) and KEY="value"; # starts a comment line.
function readEnv(text) {
  const out = {};
  for (const raw of text.split(/\r?\n/)) {
    const m = raw.trim().match(/^(?:export\s+)?([A-Za-z_][A-Za-z0-9_]*)=(.*)$/);
    if (!m) continue;
    let value = m[2].trim();
    const q = value[0];
    if ((q === "'" || q === '"') && value.length > 1 && value.endsWith(q)) value = value.slice(1, -1);
    else value = value.replace(/\s+#.*$/, "");
    out[m[1]] = value;
  }
  return out;
}

// The token is part of every request URL, so it is cut from anything printed.
function hide(text) {
  return token ? String(text).split(token).join("<token>") : String(text);
}

function fail(message, code = 2) {
  console.error(`error: ${hide(message)}`);
  process.exit(code);
}

function need(...keys) {
  const missing = keys.filter((k) => !env[k]);
  if (missing.length) fail(`${missing.join(" and ")} empty in ${envFile}. See .env.example`);
}

async function call(method, params = {}) {
  const res = await fetch(`https://api.telegram.org/bot${token}/${method}`, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify(params),
  }).catch((e) => fail(`cannot reach Telegram: ${e.cause?.code ?? e.message}`));
  const body = await res.json().catch(() => null);
  if (!body?.ok) {
    const why = body?.description ?? `${res.status} ${res.statusText}`;
    const hint =
      res.status === 401 ? " (check TELEGRAM_BOT_TOKEN)"
      : res.status === 409 ? " (the bot has a webhook, or another program is reading it)"
      : "";
    fail(`Telegram refused ${method}: ${why}${hint}`, 1);
  }
  return body.result;
}

function ensureDir() {
  mkdirSync(dir, { recursive: true, mode: 0o700 });
}

function split(text) {
  const parts = [];
  let rest = text;
  while (rest.length > MAX_TEXT) {
    let cut = rest.lastIndexOf("\n", MAX_TEXT);
    if (cut < MAX_TEXT / 2) cut = MAX_TEXT;
    parts.push(rest.slice(0, cut));
    rest = rest.slice(cut).replace(/^\n/, "");
  }
  if (rest) parts.push(rest);
  return parts;
}

function attachments(msg) {
  return ["photo", "document", "video", "voice", "audio", "sticker"].filter((k) => msg[k]);
}

function simplify(update) {
  const msg = update.message ?? update.edited_message ?? update.channel_post ?? update.edited_channel_post;
  if (!msg) return null;
  const from = msg.from
    ? {
        id: String(msg.from.id),
        name: [msg.from.first_name, msg.from.last_name].filter(Boolean).join(" "),
        username: msg.from.username ?? null,
      }
    : null;
  return {
    id: msg.message_id,
    chat_id: String(msg.chat.id),
    date: new Date(msg.date * 1000).toISOString(),
    edited: Boolean(update.edited_message ?? update.edited_channel_post),
    from,
    text: msg.text ?? msg.caption ?? "",
    reply_to: msg.reply_to_message?.message_id ?? null,
    attachments: attachments(msg),
  };
}

async function check() {
  need("TELEGRAM_BOT_TOKEN", "TELEGRAM_CHAT_ID");
  const me = await call("getMe");
  const chat = await call("getChat", { chat_id: chatId });
  console.log(`bot:   @${me.username} (${me.first_name})`);
  console.log(`chat:  ${chat.title ?? chat.username ?? chat.id} (${chat.type})`);
  console.log(
    `sees:  ${me.can_read_all_group_messages ? "every message in groups" : "only mentions, replies to it and /commands. Turn privacy mode off in @BotFather (/setprivacy), then remove the bot from the chat and add it back"}`,
  );
}

async function send(args) {
  need("TELEGRAM_BOT_TOKEN", "TELEGRAM_CHAT_ID");
  let replyTo;
  if (args[0] === "--reply") {
    replyTo = Number(args[1]);
    if (!Number.isInteger(replyTo)) fail("--reply needs a message id");
    args = args.slice(2);
  }
  let text = args.join(" ");
  if (!text || text === "-") text = readFileSync(0, "utf8");
  text = text.trim();
  if (!text) fail("nothing to send");
  const ids = [];
  for (const [i, part] of split(text).entries()) {
    const sent = await call("sendMessage", {
      chat_id: chatId,
      text: part,
      link_preview_options: { is_disabled: true },
      ...(replyTo && i === 0 ? { reply_parameters: { message_id: replyTo, allow_sending_without_reply: true } } : {}),
    });
    ids.push(sent.message_id);
  }
  console.log(`sent message ${ids.join(", ")}`);
}

async function read() {
  need("TELEGRAM_BOT_TOKEN", "TELEGRAM_CHAT_ID");
  ensureDir();
  let offset = existsSync(offsetFile) ? Number(readFileSync(offsetFile, "utf8").trim()) || 0 : 0;
  let count = 0;
  for (;;) {
    const updates = await call("getUpdates", {
      offset,
      limit: 100,
      timeout: 0,
      allowed_updates: ["message", "edited_message", "channel_post", "edited_channel_post"],
    });
    if (!updates.length) break;
    for (const u of updates) {
      const m = simplify(u);
      if (m && m.chat_id === chatId) {
        const line = JSON.stringify(m);
        appendFileSync(inboxFile, `${line}\n`, { mode: 0o600 });
        console.log(line);
        count++;
      }
      offset = u.update_id + 1;
    }
    writeFileSync(offsetFile, String(offset), { mode: 0o600 });
  }
  if (!count) console.error("no new messages");
}

function inbox(n) {
  const limit = Number(n ?? 20);
  if (!Number.isInteger(limit) || limit < 1) fail("N must be a positive number");
  if (!existsSync(inboxFile)) return console.error("the inbox is empty: nothing has been read yet");
  const lines = readFileSync(inboxFile, "utf8").split("\n").filter(Boolean);
  for (const line of lines.slice(-limit)) console.log(line);
}

const [command, ...args] = process.argv.slice(2);
switch (command) {
  case "check":
    await check();
    break;
  case "send":
    await send(args);
    break;
  case "read":
    await read();
    break;
  case "inbox":
    inbox(args[0]);
    break;
  default:
    fail("usage: telegram.mjs check | send [--reply ID] [TEXT] | read | inbox [N]");
}
