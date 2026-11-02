// Notes sur le téléphone : liste matière → chapitre → notes, puis lecture (et modification) d'une note.
import { useEffect, useMemo, useState } from "react";
import { ArrowLeft, ChevronRight, Search } from "lucide-react";
import LecteurNote from "../views/LecteurNote";
import { arbre } from "./donnees";
import { useMagasin } from "./magasin";

interface Props {
  chemin: string | null;
  onOuvrir: (chemin: string | null) => void;
  ecritureAutorisee: boolean;
}

export default function NotesMobile({ chemin, onOuvrir, ecritureAutorisee }: Props) {
  const { modele } = useMagasin();
  const [ouverts, setOuverts] = useState<Set<string>>(new Set());
  const [filtre, setFiltre] = useState("");
  const a = useMemo(() => (modele ? arbre(modele) : null), [modele]);
  // Une note ouverte (ou le retour à la liste) repart du haut de l'écran.
  useEffect(() => {
    document.querySelector(".mobile-contenu")?.scrollTo(0, 0);
  }, [chemin]);

  if (chemin) {
    return (
      <div className="notes-mobile">
        <button className="lien-discret retour-mobile" onClick={() => onOuvrir(null)}>
          <ArrowLeft size={16} aria-hidden /> Toutes les notes
        </button>
        <LecteurNote chemin={chemin} ancre={null} onOuvrir={(c) => onOuvrir(c)} versionIndex={modele?.contenu.genereLe} ecritureAutorisee={ecritureAutorisee} />
      </div>
    );
  }
  if (!a) return null;

  const basculer = (k: string) =>
    setOuverts((s) => {
      const n = new Set(s);
      if (n.has(k)) n.delete(k);
      else n.add(k);
      return n;
    });
  const recherche = filtre.trim().toLowerCase();
  const toutes = modele ? [...modele.notes.values()].filter((n) => recherche && n.titre.toLowerCase().includes(recherche)) : [];

  return (
    <div className="notes-mobile">
      <label className="recherche-mobile">
        <Search size={16} aria-hidden />
        <input type="search" placeholder="Chercher une note" value={filtre} onChange={(e) => setFiltre(e.target.value)} />
      </label>
      {recherche ? (
        <ul className="liste-notes-mobile">
          {toutes.slice(0, 50).map((n) => (
            <li key={n.chemin}>
              <button onClick={() => onOuvrir(n.chemin)}>
                <span>{n.titre}</span>
                <span className="texte-doux petit">{[n.matiere, n.chapitre].filter(Boolean).join(" › ")}</span>
              </button>
            </li>
          ))}
          {toutes.length === 0 && <li className="texte-doux petit">Aucune note trouvée.</li>}
        </ul>
      ) : (
        <ul className="liste-notes-mobile">
          {a.matieres.map((m) => (
            <li key={m.nom}>
              <button className="groupe-mobile" aria-expanded={ouverts.has(m.nom)} onClick={() => basculer(m.nom)}>
                <ChevronRight size={16} aria-hidden className={ouverts.has(m.nom) ? "tourne" : ""} /> {m.nom}
              </button>
              {ouverts.has(m.nom) && (
                <ul>
                  {m.notes.map((n) => (
                    <li key={n.chemin}>
                      <button onClick={() => onOuvrir(n.chemin)}>{n.titre}</button>
                    </li>
                  ))}
                  {m.chapitres.map((c) => {
                    const k = `${m.nom}/${c.nom}`;
                    return (
                      <li key={k}>
                        <button className="groupe-mobile" aria-expanded={ouverts.has(k)} onClick={() => basculer(k)}>
                          <ChevronRight size={16} aria-hidden className={ouverts.has(k) ? "tourne" : ""} /> {c.nom}
                        </button>
                        {ouverts.has(k) && (
                          <ul>
                            {c.notes.map((n) => (
                              <li key={n.chemin}>
                                <button onClick={() => onOuvrir(n.chemin)}>{n.titre}</button>
                              </li>
                            ))}
                          </ul>
                        )}
                      </li>
                    );
                  })}
                </ul>
              )}
            </li>
          ))}
          {a.autres
            .filter((g) => g.notes.length)
            .map((g) => (
              <li key={`autre:${g.nom}`}>
                <button className="groupe-mobile" aria-expanded={ouverts.has(`autre:${g.nom}`)} onClick={() => basculer(`autre:${g.nom}`)}>
                  <ChevronRight size={16} aria-hidden className={ouverts.has(`autre:${g.nom}`) ? "tourne" : ""} /> {g.nom || "Autres notes"}
                </button>
                {ouverts.has(`autre:${g.nom}`) && (
                  <ul>
                    {g.notes.map((n) => (
                      <li key={n.chemin}>
                        <button onClick={() => onOuvrir(n.chemin)}>{n.titre}</button>
                      </li>
                    ))}
                  </ul>
                )}
              </li>
            ))}
        </ul>
      )}
    </div>
  );
}
