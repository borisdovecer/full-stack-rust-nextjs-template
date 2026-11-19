import * as Sentry from "@sentry/react";

// Errors only, and only with a DSN. No session replay, no tracing, no personal data: a
// recording of the screen could capture a password as it is typed.
const dsn = process.env.NEXT_PUBLIC_SENTRY_DSN;

if (dsn) {
  Sentry.init({
    dsn,
    environment: process.env.NODE_ENV,
    beforeSend(event) {
      if (event.request) {
        delete event.request.cookies;
        delete event.request.data;
        delete event.request.headers;
      }
      delete event.user;
      return event;
    },
  });
}
