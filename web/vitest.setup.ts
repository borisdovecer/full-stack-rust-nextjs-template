// Adds the jest-dom matchers (toBeInTheDocument, toHaveTextContent, ...) to vitest's expect.
import "@testing-library/jest-dom/vitest";
import { cleanup } from "@testing-library/react";
import { afterEach } from "vitest";

// No vitest globals, so Testing Library cannot register its own afterEach cleanup.
afterEach(cleanup);
