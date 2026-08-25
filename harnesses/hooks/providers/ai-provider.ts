/**
 * providers/ai-provider.ts — interfaz de proveedor IA para los hooks de skills.
 *
 * Reconstruida (2026-08-25): el directorio providers/ nunca llegó al repo y
 * skill-activation-prompt / skill-verification-guard morían al importarla.
 *
 * El contrato es mínimo a propósito: texto entra, texto sale. La
 * interpretación (parseLLMJson + validación de campos) vive en los hooks.
 */

export interface AIProvider {
    /** Nombre corto para logs/métricas ("anthropic", "openai", "gemini"). */
    readonly name: string;

    /** Clasificación de prompt: devuelve la respuesta cruda del modelo. */
    classifyPrompt(prompt: string): Promise<string>;

    /** Análisis de un edit (¿qué skills exige este fichero?). */
    analyzeEdit(prompt: string): Promise<string>;

    /** Embeddings opcionales (solo proveedores con soporte). */
    embed?(text: string): Promise<number[]>;
}

/** Resultado vacío tipado: clasificar mal NUNCA debe bloquear un edit. */
export const EMPTY_CLASSIFICATION: ClassificationResult = {
    mandatory: [],
    recommended: [],
};

export interface ClassificationResult {
    mandatory: string[];
    recommended: string[];
}
