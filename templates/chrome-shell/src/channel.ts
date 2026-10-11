import type { ChannelMessage, TransportChannel } from '@almadar/runtime';

/** A Chrome runtime port as the runtime's message channel; it closes when the other end goes away (a stopped worker). */
export function portChannel(port: chrome.runtime.Port): TransportChannel {
  return {
    post: (message: ChannelMessage) => port.postMessage(message),
    onMessage: (listener) => {
      const wrapped = (message: ChannelMessage) => listener(message);
      port.onMessage.addListener(wrapped);
      return () => port.onMessage.removeListener(wrapped);
    },
    onClose: (listener) => {
      port.onDisconnect.addListener(listener);
      return () => port.onDisconnect.removeListener(listener);
    },
  };
}

export const VIEW_PORT = 'almadar-view';

/**
 * Chrome stops an extension worker after about 30 s without events; a view sends this on its port
 * more often than that, so the worker keeps running (and filtering) while the view is open.
 */
export const KEEP_ALIVE = { almadarKeepAlive: true } as const;
export const KEEP_ALIVE_MS = 20_000;
