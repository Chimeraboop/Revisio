import { beforeEach, describe, expect, it, vi } from "vitest";

const appels: { commande: string; args: unknown }[] = [];
vi.mock("@tauri-apps/api/core", () => ({
  invoke: (commande: string, args: unknown) => {
    appels.push({ commande, args });
    return Promise.resolve(true);
  },
}));

const {
  _reinitialiser,
  annulerTache,
  definirEcranActif,
  lancerTache,
  lireNotifications,
  lireTaches,
  patienter,
  verifierArret,
  MESSAGE_INTERROMPUE,
} = await import("./taches");

async function etat() {
  return { taches: lireTaches(), notifications: lireNotifications() };
}

function promesseManuelle<T>() {
  let tenir!: (v: T) => void;
  let rompre!: (e: unknown) => void;
  const promesse = new Promise<T>((a, b) => {
    tenir = a;
    rompre = b;
  });
  return { promesse, tenir, rompre };
}

describe("gestionnaire de tâches IA", () => {
  beforeEach(() => {
    _reinitialiser();
    appels.length = 0;
  });

  it("garde le résultat et prévient si l'élève regarde un autre écran", async () => {
    definirEcranActif("planning");
    const m = promesseManuelle<number>();
    lancerTache({ genre: "questions", titre: "Questions · Méiose", ecran: "atelier", resume: (n) => `${n} questions` }, () => m.promesse);
    expect((await etat()).taches[0].statut).toBe("en-cours");
    m.tenir(8);
    await m.promesse;
    await Promise.resolve();
    const { taches, notifications } = await etat();
    expect(taches[0]).toMatchObject({ statut: "finie", resultat: 8, resume: "8 questions" });
    expect(notifications.map((t) => t.titre)).toEqual(["Questions · Méiose"]);
    // En ouvrant l'écran concerné, la notification disparaît.
    definirEcranActif("atelier");
    expect((await etat()).notifications).toEqual([]);
  });

  it("pas de notification quand on est déjà sur l'écran de la tâche", async () => {
    definirEcranActif("atelier");
    await lancerTache({ genre: "questions", titre: "Q", ecran: "atelier" }, async () => 1);
    await Promise.resolve();
    expect((await etat()).notifications).toEqual([]);
  });

  it("une erreur est gardée avec son message", async () => {
    definirEcranActif("accueil");
    await lancerTache({ genre: "feynman", titre: "F", ecran: "atelier" }, async () => {
      throw "Gemini est surchargé";
    }).catch(() => {});
    await Promise.resolve();
    const { taches, notifications } = await etat();
    expect(taches[0]).toMatchObject({ statut: "erreur", erreur: "Gemini est surchargé" });
    expect(notifications).toHaveLength(1);
  });

  it("Arrêter prévient le cœur Rust et marque la tâche interrompue, sans notification", async () => {
    definirEcranActif("accueil");
    const m = promesseManuelle<number>();
    lancerTache({ genre: "podcast", titre: "P", ecran: "podcasts" }, () => m.promesse).catch(() => {});
    const id = (await etat()).taches[0].id;
    annulerTache(id);
    expect(appels).toEqual([{ commande: "annuler_tache", args: { id } }]);
    m.rompre(MESSAGE_INTERROMPUE);
    await m.promesse.catch(() => {});
    await Promise.resolve();
    const { taches, notifications } = await etat();
    expect(taches[0].statut).toBe("interrompue");
    expect(notifications).toEqual([]);
  });

  it("une génération en plusieurs étapes s'arrête entre deux étapes", async () => {
    vi.useFakeTimers();
    let etapes = 0;
    const promesse = lancerTache({ genre: "video", titre: "V", ecran: "videos" }, async (c) => {
      for (;;) {
        verifierArret(c);
        etapes++;
        await patienter(c, 2, () => "attente");
      }
    });
    const id = (await etat()).taches[0].id;
    await vi.advanceTimersByTimeAsync(2500);
    annulerTache(id);
    await vi.advanceTimersByTimeAsync(2000);
    await expect(promesse).rejects.toThrow(MESSAGE_INTERROMPUE);
    expect(etapes).toBe(2);
    vi.useRealTimers();
  });
});
