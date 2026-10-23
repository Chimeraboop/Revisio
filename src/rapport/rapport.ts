// Rapport de progression en Markdown (fonction pure, testée dans rapport.test.ts).
// Le même texte est affiché dans l'app, enregistré en .md, ou imprimé en PDF.
import type { CarteStat, ErreurCarnet, Evaluation, JourActivite, NoteCopie, NotionFaible } from "../lib/api";
import { NOMS_EVALUATIONS } from "../lib/api";
import { compteARebours, ecartJours, libelleJour } from "../planning/planning";
import { NOMS_GENRES } from "../revision/modules";
import {
  derniersJours,
  formaterDuree,
  jourLocal,
  LIBELLES_STATUT,
  maitriseParMatiere,
  serieDeJours,
  statut,
  totaux,
  type Maitrise,
} from "../revision/stats";

export interface DonneesRapport {
  maintenant: Date;
  cartes: CarteStat[];
  jours: JourActivite[];
  faibles: NotionFaible[];
  evaluations: Evaluation[];
  notes: NoteCopie[];
  /** Erreurs pas encore comprises. */
  erreurs: ErreurCarnet[];
}

const formatDate = new Intl.DateTimeFormat("fr-FR", { weekday: "long", day: "numeric", month: "long", year: "numeric" });
const formatCourt = new Intl.DateTimeFormat("fr-FR", { day: "numeric", month: "short" });
const formatNote = new Intl.NumberFormat("fr-FR", { maximumFractionDigits: 1 });

/** Texte sûr dans une cellule de tableau Markdown. */
function cellule(t: string): string {
  return t.replace(/\|/g, "\\|").replace(/\s*\n\s*/g, " ").trim();
}

const ICONES = { solide: "✓", fragile: "!", "a-revoir": "✗", "pas-commence": "○" } as const;

function maitrise(m: Maitrise): string {
  const s = statut(m);
  return `${ICONES[s]} ${LIBELLES_STATUT[s]}${m.vues > 0 ? ` · ${Math.round(m.moyenne * 100)} %` : ""}`;
}

export function rapportMarkdown(d: DonneesRapport): string {
  const lignes: string[] = [];
  const aujourdhui = jourLocal(d.maintenant);
  const semaine = totaux(derniersJours(d.jours, 7, d.maintenant));
  const total = totaux(d.jours);
  const vues = d.cartes.filter((c) => c.etatFsrs).length;

  lignes.push("# Ma progression — Révisio", "", `*Rapport du ${formatDate.format(d.maintenant)}*`, "");
  lignes.push("## En bref", "");
  const serie = serieDeJours(d.jours, d.maintenant);
  lignes.push(`- **Série :** ${serie} jour${serie > 1 ? "s" : ""} de révision d'affilée`);
  lignes.push(
    `- **Réussite sur 7 jours :** ${
      semaine.revisions ? `${Math.round((semaine.justes / semaine.revisions) * 100)} %` : "—"
    } (${semaine.revisions} réponse${semaine.revisions > 1 ? "s" : ""})`,
  );
  lignes.push(`- **Temps de révision :** ${formaterDuree(semaine.dureeMs)} sur 7 jours, ${formaterDuree(total.dureeMs)} au total`);
  lignes.push(`- **Cartes déjà vues :** ${vues} sur ${d.cartes.length}`, "");

  if (d.evaluations.length) {
    lignes.push("## Prochaines évaluations", "");
    for (const e of d.evaluations) {
      const chapitres = e.chapitres.map((c) => c ?? "notes générales").join(", ");
      // « dans 5 jours » seulement au-delà de demain (sinon « Aujourd'hui (aujourd'hui) »).
      const delai = ecartJours(aujourdhui, e.jour) > 1 ? ` (${compteARebours(e.jour, aujourdhui)})` : "";
      lignes.push(
        `- **${e.titre}** — ${libelleJour(e.jour, aujourdhui)}${delai} · ${NOMS_EVALUATIONS[e.genre]} · ${chapitres}`,
      );
    }
    lignes.push("");
  }

  const matieres = maitriseParMatiere(d.cartes, d.maintenant);
  if (matieres.length) {
    lignes.push("## Maîtrise par matière", "");
    lignes.push("Probabilité moyenne de se souvenir des cartes aujourd'hui (une carte jamais vue compte 0).", "");
    lignes.push("| Matière | Maîtrise | Cartes vues |", "|---|---|---|");
    for (const m of matieres) lignes.push(`| ${cellule(m.matiere)} | ${maitrise(m)} | ${m.vues} / ${m.total} |`);
    lignes.push("", "### Par chapitre", "", "| Matière | Chapitre | Maîtrise | Cartes vues |", "|---|---|---|---|");
    for (const m of matieres) {
      for (const c of m.chapitres) {
        lignes.push(`| ${cellule(m.matiere)} | ${cellule(c.chapitre)} | ${maitrise(c)} | ${c.vues} / ${c.total} |`);
      }
    }
    lignes.push("");
  }

  if (d.notes.length) {
    const moyenne = d.notes.reduce((t, n) => t + n.note, 0) / d.notes.length;
    lignes.push("## Exercices et examens", "");
    lignes.push(`Moyenne des ${d.notes.length} dernières copies : **${formatNote.format(moyenne)} / 20**.`, "");
    lignes.push("| Date | Sujet | Type | Matière | Note |", "|---|---|---|---|---|");
    for (const n of d.notes) {
      lignes.push(
        `| ${formatCourt.format(new Date(n.date))} | ${cellule(n.titre)} | ${cellule(n.typeNom)} | ${cellule(n.matiere)} | ${formatNote.format(n.note)} / 20 |`,
      );
    }
    lignes.push("");
  }

  if (d.faibles.length) {
    lignes.push("## Notions les plus faibles", "");
    for (const f of d.faibles) {
      const libelle = f.genre === "trou" ? f.reponse : f.genre === "ordre" ? f.section || f.question : f.question;
      const lieu = [f.matiere, f.chapitre].filter(Boolean).join(" › ");
      lignes.push(
        `- **${cellule(libelle)}** (${NOMS_GENRES[f.genre] ?? f.genre}${lieu ? ` · ${lieu}` : ""}) : ${f.erreurs} erreur${
          f.erreurs > 1 ? "s" : ""
        }, ${f.oublis} oubli${f.oublis > 1 ? "s" : ""}`,
      );
    }
    lignes.push("");
  }

  lignes.push("## Carnet d'erreurs", "");
  if (!d.erreurs.length) {
    lignes.push("Aucune erreur à retravailler.");
  } else {
    const parMatiere = new Map<string, number>();
    for (const e of d.erreurs) parMatiere.set(e.matiere ?? "Hors matière", (parMatiere.get(e.matiere ?? "Hors matière") ?? 0) + 1);
    lignes.push(`${d.erreurs.length} erreur${d.erreurs.length > 1 ? "s" : ""} à retravailler :`, "");
    for (const [m, n] of [...parMatiere.entries()].sort((a, b) => b[1] - a[1])) lignes.push(`- ${m} : ${n}`);
  }
  lignes.push("");
  return lignes.join("\n");
}

/** Nom du fichier exporté : progression-AAAA-MM-JJ.md */
export function nomFichier(maintenant: Date): string {
  return `progression-${jourLocal(maintenant)}.md`;
}
