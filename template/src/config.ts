/**
 * Build-time configuration of the knowledge source (VITE_ env at build). Absent, the `knowledge`
 * service is not wired and calls to it fail with NOT_CONFIGURED.
 */
export interface KnowledgeConfig {
  baseUrl: string;
  apiKey: string;
  googleClientId: string;
}

const env = import.meta.env;

export const knowledgeConfig: KnowledgeConfig | null =
  env.VITE_KNOWLEDGE_BASE_URL && env.VITE_KNOWLEDGE_FIREBASE_API_KEY && env.VITE_KNOWLEDGE_GOOGLE_CLIENT_ID
    ? { baseUrl: env.VITE_KNOWLEDGE_BASE_URL, apiKey: env.VITE_KNOWLEDGE_FIREBASE_API_KEY, googleClientId: env.VITE_KNOWLEDGE_GOOGLE_CLIENT_ID }
    : null;
