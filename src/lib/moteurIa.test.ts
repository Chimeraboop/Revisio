import { describe, expect, it } from "vitest";
import { choisirMoteur, modeleInstalle, nomModele } from "./moteurIa";

describe("choix de l'IA pour les tâches simples", () => {
  it("mode secours : Gemini d'abord, l'IA locale seulement quand Gemini est indisponible", () => {
    expect(choisirMoteur("secours", true, "qwen3.5:4b")).toEqual({ simple: true, local: false, badge: null });
    expect(choisirMoteur("secours", false, "qwen3.5:4b")).toMatchObject({ simple: true, local: true });
    expect(choisirMoteur("secours", false, null)).toEqual({ simple: false, local: false, badge: null });
  });

  it("mode toujours : l'IA locale, même en ligne ; indisponible si Ollama ne répond pas", () => {
    expect(choisirMoteur("toujours", true, "ministral-3:8b")).toMatchObject({ simple: true, local: true });
    expect(choisirMoteur("toujours", true, null)).toMatchObject({ simple: false, local: false });
  });

  it("mode jamais : comme avant (Gemini seulement)", () => {
    expect(choisirMoteur("jamais", false, "qwen3.5:4b")).toEqual({ simple: false, local: false, badge: null });
    expect(choisirMoteur("jamais", true, null).simple).toBe(true);
  });

  it("modèle installé et nom affiché", () => {
    expect(modeleInstalle("qwen3.5:4b", ["qwen3.5:4b"])).toBe(true);
    expect(modeleInstalle("llama3", ["llama3:latest"])).toBe(true);
    expect(modeleInstalle("qwen3.5:9b", ["qwen3.5:4b"])).toBe(false);
    expect(modeleInstalle("", ["qwen3.5:4b"])).toBe(false);
    expect(nomModele("local:qwen3.5:4b")).toBe("IA locale (qwen3.5:4b)");
    expect(nomModele("gemini-3.8-flash")).toBe("Gemini (gemini-3.8-flash)");
  });
});
