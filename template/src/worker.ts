/**
 * The extension's service worker hosts the program: browser-stored entities in IndexedDB, every
 * `call-service` answered by the browser integrations, declared inputs dispatched by the runtime.
 * At start it mounts the program's `/` page, as a view would, so its traits run with no view open;
 * the side panel connects over a port and renders the same running program.
 */
import { getTraitName, parseOrbitalSchema } from '@almadar/core';
import { browserStoreName, holdUntilServed, openBrowserHost, serveChannel, type BrowserHost, type EventTransport } from '@almadar/runtime';
import {
  chromePageHost,
  chromePromptModel,
  createBrowserCallService,
  createBrowserLlm,
  createKnowledgeService,
  createPageService,
  firebaseKnowledgeAuth,
  type BrowserServices,
  type ChromeLanguageModel,
} from '@almadar/integrations/browser';
import programJson from './generated/program.json';
import { portChannel, VIEW_PORT } from './channel';
import { knowledgeConfig } from './config';
import { embedder } from './generated/embedder';

declare const LanguageModel: ChromeLanguageModel | undefined;

const schema = parseOrbitalSchema(programJson);

const services: BrowserServices = {
  page: createPageService(chromePageHost(chrome.tabs, chrome.runtime)),
  llm: createBrowserLlm(chromePromptModel(() => (typeof LanguageModel === 'undefined' ? undefined : LanguageModel)), embedder),
};
if (knowledgeConfig) {
  services.knowledge = createKnowledgeService({
    baseUrl: knowledgeConfig.baseUrl,
    fetch: (input, init) => fetch(input, init),
    auth: firebaseKnowledgeAuth({
      apiKey: knowledgeConfig.apiKey,
      googleClientId: knowledgeConfig.googleClientId,
      identity: chrome.identity,
      storage: chrome.storage.local,
      fetch: (input, init) => fetch(input, init),
    }),
  });
}

const hostReady: Promise<BrowserHost> = openBrowserHost({
  databaseName: browserStoreName(schema.name, undefined),
  schema,
  callService: createBrowserCallService(services),
});

async function mountRootPage(host: EventTransport): Promise<void> {
  for (const orbital of schema.orbitals) {
    for (const page of orbital.pages ?? []) {
      if (typeof page === 'string' || page.path !== '/') continue;
      for (const ref of page.traits ?? []) {
        await host.send(orbital.name, { event: 'INIT', targetTrait: getTraitName(ref), payload: {} });
      }
    }
  }
}

void hostReady.then(mountRootPage);

chrome.runtime.onConnect.addListener((port) => {
  if (port.name !== VIEW_PORT) return;
  // A view can connect while the worker is still starting; its first messages wait for the host.
  const channel = holdUntilServed(portChannel(port));
  void hostReady.then((host) => {
    const served = serveChannel(channel, host, { schemaName: schema.name });
    // What the worker runs on its own (a watched page's elements) shows in the open panel.
    const stopReporting = host.onInputDispatched((orbital, request, response) => served.pushDispatch(orbital, request, response));
    port.onDisconnect.addListener(() => {
      stopReporting();
      served.stop();
    });
  });
});

void chrome.sidePanel.setPanelBehavior({ openPanelOnActionClick: true });
