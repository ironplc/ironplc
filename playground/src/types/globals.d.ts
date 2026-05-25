// Globals injected by third-party scripts loaded before app.ts.
//
// - `clicky` is set by https://static.getclicky.com/js (loaded with `async`).
// - `posthog` is set by the inline snippet in `posthog-init.js`, which queues
//   calls until the real SDK loads asynchronously.

interface Clicky {
  log(path: string, title: string): void;
}

interface PostHog {
  capture(event: string, props?: Record<string, unknown>): void;
  register(props: Record<string, unknown>): void;
}

declare global {
  // eslint-disable-next-line no-var
  var clicky: Clicky | undefined;
  // eslint-disable-next-line no-var
  var posthog: PostHog | undefined;
}

export {};
