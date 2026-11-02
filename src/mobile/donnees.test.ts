import { describe, expect, it } from "vitest";
import {
  alleger,
  arbre,
  carnet,
  construire,
  enregistrerNote,
  enregistrerRevision,
  journalVide,
  marquerComprise,
  preparerSeance,
  resoudre,
  resume,
  revuesDuJour,
  suspendre,
  type CarteSynchro,
  type Contenu,
} from "./donnees";

const JOUR = 86_400_000;
const MAINTENANT = 100 * JOUR;

function carte(cle: string, autres: Partial<CarteSynchro> = {}): CarteSynchro {
  return {
    cle,
    genre: "trou",
    noteChemin: "SVT/Méiose/La méiose.md",
    matiere: "SVT",
    chapitre: "Méiose",
    section: "",
    question: `q ${cle}`,
    reponse: `r ${cle}`,
    donnees: {},
    extrait: "",
    suspendue: false,
    creeeLe: 0,
    etatFsrs: null,
    echeance: null,
    introduiteLe: null,
    etatMaj: 0,
    ...autres,
  };
}

function contenu(): Contenu {
  return {
    version: 1,
    appareil: "pc-1",
    genereLe: MAINTENANT,
    reglages: {
      nouvellesParJour: 2,
      nouvellesParJourLangues: 1,
      dureeSeance: 20,
      dureeSeanceLangues: 10,
      entrelacement: true,
      vocabVersFrancais: "retourner",
      vocabDepuisFrancais: "taper",
      sensVocabulaire: "les_deux",
      matieresExclues: [],
      ecritureAutorisee: true,
    },
    cartes: [
      carte("due", { etatFsrs: '{"lapses":0}', echeance: MAINTENANT - 1000, etatMaj: 50 * JOUR }),
      carte("pas-due", { etatFsrs: '{"lapses":2}', echeance: MAINTENANT + JOUR, etatMaj: 50 * JOUR }),
      carte("n1"),
      carte("n2"),
      carte("n3"),
      carte("vocab", { genre: "vocabulaire", matiere: "Allemand", chapitre: "Wortschatz", donnees: { sens: "vers_fr" } }),
      carte("vocab-fr", { genre: "vocabulaire", matiere: "Allemand", chapitre: "Wortschatz", donnees: { sens: "depuis_fr" } }),
    ],
    revisions: [{ uid: "pc-1:r1", cle: "due", date: 50 * JOUR, note: 3, confiance: "sur", correcte: true, dureeMs: 10_000 }],
    erreurs: [],
    evaluations: [],
    notes: [
      {
        chemin: "SVT/Méiose/La méiose.md",
        nom: "La méiose",
        titre: "La méiose",
        matiere: "SVT",
        chapitre: "Méiose",
        genre: "cours",
        tags: [],
        frontmatter: null,
        contenu: "Voir [[Mitose]].\r\nFin",
        empreinte: "abc-20",
      },
      {
        chemin: "SVT/Mitose/Mitose.md",
        nom: "Mitose",
        titre: "Mitose",
        matiere: "SVT",
        chapitre: "Mitose",
        genre: "cours",
        tags: [],
        frontmatter: null,
        contenu: "Texte",
        empreinte: "def-5",
      },
    ],
    notesTraitees: [],
  };
}

const debut = MAINTENANT - (MAINTENANT % JOUR);

describe("séances sur le téléphone", () => {
  it("dues, faibles, nouvelles dans la limite du quota", () => {
    const m = construire(contenu(), journalVide("iphone-1"));
    const s = preparerSeance(m, { debutJour: debut, groupe: "autres" }, MAINTENANT, m.contenu.reglages);
    expect(s.dues.map((c) => c.question)).toEqual(["q due"]);
    expect(s.faibles.map((c) => c.question)).toEqual(["q pas-due"]);
    expect(s.nouvelles.map((c) => c.question)).toEqual(["q n1", "q n2"]);
    const r = resume(m, MAINTENANT, debut, m.contenu.reglages);
    expect(r.groupes.find((g) => g.groupe === "autres")).toMatchObject({ dues: 1, nouvellesDisponibles: 3, nouvellesRestantes: 2 });
  });

  it("recto du vocabulaire et matières exclues", () => {
    const c = contenu();
    c.reglages.sensVocabulaire = "francais";
    const m = construire(c, journalVide("iphone-1"));
    const s = preparerSeance(m, { debutJour: debut, groupe: "langues" }, MAINTENANT, c.reglages);
    expect(s.nouvelles.map((x) => x.question)).toEqual(["q vocab-fr"]);
    c.reglages.matieresExclues = ["Allemand"];
    const m2 = construire(c, journalVide("iphone-1"));
    expect(preparerSeance(m2, { debutJour: debut, groupe: "langues" }, MAINTENANT, c.reglages).nouvelles).toEqual([]);
    // Séance ciblée : les matières exclues ne comptent pas.
    expect(preparerSeance(m2, { debutJour: debut, genres: ["vocabulaire"] }, MAINTENANT, c.reglages).nouvelles).toHaveLength(1);
  });

  it("une réponse met à jour l'état, le quota et le carnet", () => {
    let m = construire(contenu(), journalVide("iphone-1"));
    const n1 = m.cartes.find((c) => c.cle === "n1")!;
    const j = enregistrerRevision(
      m,
      { carteId: n1.id, note: 1, confiance: "hesitant", correcte: false, reponseDonnee: "x", dureeMs: 4000, etatFsrs: '{"lapses":1}', echeance: MAINTENANT + 600_000 },
      MAINTENANT,
    );
    m = construire(m.contenu, j);
    const apres = m.parCle.get("n1")!;
    expect(apres.etatFsrs).toBe('{"lapses":1}');
    expect(apres.introduiteLe).toBe(MAINTENANT);
    // Une nouvelle carte commencée : il n'en reste qu'une pour aujourd'hui.
    expect(preparerSeance(m, { debutJour: debut, groupe: "autres" }, MAINTENANT, m.contenu.reglages).nouvelles).toHaveLength(1);
    const erreurs = carnet(m, null, false);
    expect(erreurs).toHaveLength(1);
    expect(erreurs[0]).toMatchObject({ question: "q n1", reponseDonnee: "x", titreNote: "La méiose" });
    expect(revuesDuJour(m, debut)).toEqual([{ matiere: "SVT", chapitre: "Méiose", cartes: 1 }]);
    // « J'ai compris » puis carnet sans les erreurs comprises.
    m = construire(m.contenu, marquerComprise(m, erreurs[0].id, true, MAINTENANT + 1));
    expect(carnet(m, null, false)).toHaveLength(0);
    expect(carnet(m, null, true)[0].comprise).toBe(true);
  });

  it("l'état du PC gagne s'il est plus récent", () => {
    const c = contenu();
    const j = journalVide("iphone-1");
    j.etats = [{ cle: "due", etatFsrs: '{"vieux":1}', echeance: 1, introduiteLe: null, maj: 40 * JOUR }];
    expect(construire(c, j).parCle.get("due")!.etatFsrs).toBe('{"lapses":0}');
    j.etats[0].maj = 60 * JOUR;
    expect(construire(c, j).parCle.get("due")!.etatFsrs).toBe('{"vieux":1}');
  });

  it("carte signalée sur le téléphone", () => {
    const m = construire(contenu(), journalVide("iphone-1"));
    const j = suspendre(m, m.parCle.get("due")!.id);
    const m2 = construire(m.contenu, j);
    expect(preparerSeance(m2, { debutJour: debut }, MAINTENANT, m2.contenu.reglages).dues).toEqual([]);
  });
});

describe("notes sur le téléphone", () => {
  it("arbre et liens", () => {
    const m = construire(contenu(), journalVide("iphone-1"));
    expect(arbre(m).matieres[0].chapitres.map((c) => c.nom).sort()).toEqual(["Mitose", "Méiose"].sort());
    expect(resoudre(m, "Mitose", "SVT/Méiose/La méiose.md")).toEqual({ chemin: "SVT/Mitose/Mitose.md", genre: "note" });
    expect(resoudre(m, "Inconnue", null)).toBeNull();
  });

  it("modification : retours à la ligne gardés, version de départ envoyée au PC", () => {
    const m = construire(contenu(), journalVide("iphone-1"));
    const j = enregistrerNote(m, "SVT/Méiose/La méiose.md", "Voir [[Mitose]].\nFin\nAjout", "Voir [[Mitose]].\nFin", 5);
    expect(j.notes).toEqual([{ chemin: "SVT/Méiose/La méiose.md", base: "abc-20", contenu: "Voir [[Mitose]].\r\nFin\r\nAjout", date: 5 }]);
    const m2 = construire(m.contenu, j);
    expect(m2.notes.get("SVT/Méiose/La méiose.md")!.contenu).toContain("Ajout");
    // Version périmée : refusée.
    expect(() => enregistrerNote(m2, "SVT/Méiose/La méiose.md", "autre", "Voir [[Mitose]].\nFin", 6)).toThrow();
    // Retour au texte d'origine : plus rien à envoyer.
    expect(enregistrerNote(m2, "SVT/Méiose/La méiose.md", "Voir [[Mitose]].\nFin", "Voir [[Mitose]].\nFin\nAjout", 7).notes).toEqual([]);
  });
});

describe("après une synchronisation", () => {
  it("le journal oublie ce que le PC a déjà fusionné", () => {
    const c = contenu();
    let m = construire(c, journalVide("iphone-1"));
    const id = m.parCle.get("n1")!.id;
    let j = enregistrerRevision(m, { carteId: id, note: 3, confiance: "sur", correcte: true, reponseDonnee: null, dureeMs: 1, etatFsrs: "{}", echeance: 9 }, MAINTENANT);
    j = enregistrerNote(construire(c, j), "SVT/Mitose/Mitose.md", "Texte modifié", "Texte", MAINTENANT);
    // Le PC a fusionné : révision connue, état à la même date, note proposée à l'élève (traitée).
    const c2 = contenu();
    c2.revisions.push({ ...j.revisions[0] });
    c2.cartes = c2.cartes.map((x) => (x.cle === "n1" ? { ...x, etatFsrs: "{}", echeance: 9, etatMaj: MAINTENANT } : x));
    c2.notesTraitees = [`iphone-1|SVT/Mitose/Mitose.md|${MAINTENANT}`];
    const leger = alleger(j, c2);
    expect(leger.revisions).toEqual([]);
    expect(leger.etats).toEqual([]);
    expect(leger.notes).toEqual([]);
    m = construire(c2, leger);
    expect(m.parCle.get("n1")!.etatFsrs).toBe("{}");
  });
});
