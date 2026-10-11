import { useEffect, useMemo, useState } from 'react';
import { createRoot } from 'react-dom/client';
import { parseOrbitalSchema } from '@almadar/core';
import { createChannelTransport } from '@almadar/runtime';
import { OrbPreview } from '@almadar/ui/runtime';
import programJson from './generated/program.json';
import { KEEP_ALIVE, KEEP_ALIVE_MS, portChannel, VIEW_PORT } from './channel';
import './index.css';

const schema = parseOrbitalSchema(programJson);

// The worker runs the whole program, browser-stored entities included; the panel only shows it.
// If its port closes anyway (the worker stopped or updated), the panel connects again and shows the
// program afresh, so it never waits on a worker that is gone.
function Panel() {
  const [connection, setConnection] = useState(0);
  const port = useMemo(() => chrome.runtime.connect({ name: VIEW_PORT }), [connection]);
  const transport = useMemo(() => createChannelTransport(portChannel(port), { hostsBrowserStore: true }), [port]);
  useEffect(() => {
    const keepAlive = setInterval(() => port.postMessage(KEEP_ALIVE), KEEP_ALIVE_MS);
    const reconnect = () => setConnection((n) => n + 1);
    port.onDisconnect.addListener(reconnect);
    return () => {
      clearInterval(keepAlive);
      port.onDisconnect.removeListener(reconnect);
      port.disconnect();
    };
  }, [port]);
  return <OrbPreview key={connection} schema={schema} transport={transport} initialPagePath="/" height="100vh" />;
}

const root = document.getElementById('root');
if (root) createRoot(root).render(<Panel />);
