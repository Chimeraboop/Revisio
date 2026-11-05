import { describe, expect, it } from "vitest";
import { auLancement, auRetour, ECART_RENOUVELLEMENT } from "./lancement";

const base = { connecte: true, enLigne: true, jetonValable: false, maintenant: 1_000_000_000 };

describe("au lancement", () => {
  it("renouvelle la connexion expirée", () => {
    expect(auLancement({ ...base, refusGoogle: false, dernierRenouvellement: null })).toBe("renouveler");
  });
  it("synchronise directement si la connexion est encore valable", () => {
    expect(auLancement({ ...base, jetonValable: true, refusGoogle: false, dernierRenouvellement: null })).toBe("synchroniser");
  });
  it("ne boucle pas : pas de second renouvellement juste après le premier, ni après un refus de Google", () => {
    expect(auLancement({ ...base, refusGoogle: false, dernierRenouvellement: base.maintenant - 30_000 })).toBe("attendre");
    expect(auLancement({ ...base, refusGoogle: true, dernierRenouvellement: null })).toBe("attendre");
    expect(auLancement({ ...base, refusGoogle: false, dernierRenouvellement: base.maintenant - ECART_RENOUVELLEMENT - 1 })).toBe(
      "renouveler",
    );
  });
  it("attend sans Internet ou sans compte connecté", () => {
    expect(auLancement({ ...base, enLigne: false, refusGoogle: false, dernierRenouvellement: null })).toBe("attendre");
    expect(auLancement({ ...base, connecte: false, refusGoogle: false, dernierRenouvellement: null })).toBe("attendre");
  });
});

describe("au retour dans l'app", () => {
  it("synchronise si la connexion est valable, signale sinon (jamais de départ chez Google en pleine séance)", () => {
    expect(auRetour({ ...base, jetonValable: true, derniere: null, enCours: false })).toBe("synchroniser");
    expect(auRetour({ ...base, derniere: null, enCours: false })).toBe("signaler");
  });
  it("ne refait rien juste après une synchronisation, ni pendant une synchronisation", () => {
    expect(auRetour({ ...base, jetonValable: true, derniere: base.maintenant - 30_000, enCours: false })).toBe("rien");
    expect(auRetour({ ...base, jetonValable: true, derniere: null, enCours: true })).toBe("rien");
  });
});
