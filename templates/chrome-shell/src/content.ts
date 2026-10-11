// The page side of the `page` integration: one generic executor over this tab's document.
import { runPageContentScript } from '@almadar/integrations/browser/content';

runPageContentScript(chrome.runtime, document, location.href);
