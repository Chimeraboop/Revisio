// Espace web : onglets de navigation dans Révisio. Les pages s'affichent dans une zone de la fenêtre
// (webviews natives posées par-dessus) ; elles n'ont aucun accès à l'app et n'écrivent rien dans le vault.
// Un passage sélectionné dans une page peut être enregistré comme source d'un chapitre.
import { listen } from "@tauri-apps/api/event";
import { useEffect, useMemo, useRef, useState } from "react";
import {
  ArrowLeft,
  ArrowRight,
  BookmarkPlus,
  ExternalLink,
  House,
  PanelRightClose,
  PanelRightOpen,
  Plus,
  RotateCw,
  TextSelect,
  X,
} from "lucide-react";
import { api, ChapitreIa, Favori, messageErreur, Onglet, SourceWeb, ZoneWeb } from "../lib/api";
import { useEnLigne } from "../lib/connexion";

const RACCOURCIS: Favori[] = [
  { id: -1, matiere: null, titre: "Gemini", url: "https://gemini.google.com/app" },
  { id: -2, matiere: null, titre: "NotebookLM", url: "https://notebooklm.google.com" },
  { id: -3, matiere: null, titre: "Wikipédia", url: "https://fr.wikipedia.org" },
  { id: -4, matiere: null, titre: "Lumni", url: "https://www.lumni.fr" },
];

const cle = (c: { matiere: string; chapitre: string | null }) => `${c.matiere}|${c.chapitre ?? ""}`;

function libelle(o: Onglet): string {
  if (o.titre) return o.titre;
  try {
    return new URL(o.url).hostname.replace(/^www\./, "");
  } catch {
    return o.url;
  }
}

export default function EspaceWebVue() {
  const enLigne = useEnLigne();
  const zone = useRef<HTMLDivElement>(null);
  const [onglets, setOnglets] = useState<Onglet[]>([]);
  const [adresse, setAdresse] = useState("");
  const [panneau, setPanneau] = useState(true);
  const [favoris, setFavoris] = useState<Favori[]>([]);
  const [sources, setSources] = useState<SourceWeb[]>([]);
  const [chapitres, setChapitres] = useState<ChapitreIa[]>([]);
  const [brouillon, setBrouillon] = useState<SourceWeb | null>(null);
  const [nouveauFavori, setNouveauFavori] = useState<Favori | null>(null);
  const [message, setMessage] = useState<{ texte: string; erreur?: boolean } | null>(null);

  const actif = onglets.find((o) => o.actif) ?? null;
  const matieres = useMemo(() => [...new Set(chapitres.map((c) => c.matiere))], [chapitres]);

  const mesurer = (): ZoneWeb | null => {
    const r = zone.current?.getBoundingClientRect();
    return r ? { x: r.left, y: r.top, largeur: Math.max(r.width, 1), hauteur: Math.max(r.height, 1) } : null;
  };

  const chargerListes = () => {
    api.favoris().then(setFavoris).catch(() => {});
    api.sourcesWeb(null).then(setSources).catch(() => {});
  };

  useEffect(() => {
    chargerListes();
    api.chapitresIa().then(setChapitres).catch(() => {});
    api.webOnglets().then((l) => {
      setOnglets(l);
      const z = mesurer();
      if (z && l.some((o) => o.actif)) api.webZone(z);
    });
    const abonnements = [
      listen<Onglet[]>("web-onglets", (e) => setOnglets(e.payload)),
      listen<string>("web-nouvel-onglet", (e) => ouvrir(e.payload)),
      listen<[boolean, string | null]>("web-telechargement", (e) =>
        setMessage(
          e.payload[0]
            ? { texte: `Téléchargé dans ton dossier Téléchargements${e.payload[1] ? ` : ${e.payload[1]}` : ""}.` }
            : { texte: "Le téléchargement a échoué.", erreur: true },
        ),
      ),
    ];
    // Les pages suivent la zone quand la fenêtre change de taille.
    const observateur = new ResizeObserver(() => {
      const z = mesurer();
      if (z) api.webZone(z).catch(() => {});
    });
    if (zone.current) observateur.observe(zone.current);
    return () => {
      observateur.disconnect();
      abonnements.forEach((a) => a.then((arreter) => arreter()));
      // En quittant l'espace web, les pages sont masquées (elles restent ouvertes).
      api.webMasquer().catch(() => {});
    };
  }, []);

  // L'adresse affichée suit l'onglet actif.
  useEffect(() => {
    setAdresse(actif?.url ?? "");
  }, [actif?.id, actif?.url]);

  // Le panneau ouvert ou fermé change la taille de la zone.
  useEffect(() => {
    const z = mesurer();
    if (z && actif) api.webZone(z).catch(() => {});
  }, [panneau]);

  async function ouvrir(cible: string) {
    const z = mesurer();
    if (!z) return;
    setMessage(null);
    try {
      await api.webOuvrir(cible, z);
    } catch (e) {
      setMessage({ texte: messageErreur(e), erreur: true });
    }
  }

  async function aller() {
    if (!adresse.trim()) return;
    if (actif) {
      try {
        await api.webNaviguer(actif.id, adresse);
      } catch (e) {
        setMessage({ texte: messageErreur(e), erreur: true });
      }
    } else ouvrir(adresse);
  }

  async function capturer() {
    if (!actif) return;
    setMessage(null);
    try {
      const texte = (await api.webSelection(actif.id)).trim();
      if (!texte) {
        setMessage({ texte: "Sélectionne d'abord un passage dans la page (à la souris), puis clique de nouveau." });
        return;
      }
      // Chapitre proposé : celui de la dernière source enregistrée, sinon le premier de la liste.
      const dernier = sources[0];
      const defaut = (dernier && chapitres.find((c) => cle(c) === cle(dernier))) ?? chapitres[0];
      setBrouillon({
        id: 0,
        date: 0,
        url: actif.url,
        titre: actif.titre,
        texte,
        matiere: defaut?.matiere ?? "",
        chapitre: defaut?.chapitre ?? null,
      });
    } catch (e) {
      setMessage({ texte: messageErreur(e), erreur: true });
    }
  }

  async function enregistrerSource() {
    if (!brouillon) return;
    try {
      await api.ajouterSourceWeb(brouillon);
      setBrouillon(null);
      setMessage({ texte: "Passage enregistré comme source (dans Révisio, pas dans ton vault)." });
      chargerListes();
    } catch (e) {
      setMessage({ texte: messageErreur(e), erreur: true });
    }
  }

  async function enregistrerFavori() {
    if (!nouveauFavori) return;
    try {
      await api.ajouterFavori(nouveauFavori);
      setNouveauFavori(null);
      chargerListes();
    } catch (e) {
      setMessage({ texte: messageErreur(e), erreur: true });
    }
  }

  const parMatiere = new Map<string, Favori[]>();
  for (const f of favoris) parMatiere.set(f.matiere ?? "Général", [...(parMatiere.get(f.matiere ?? "Général") ?? []), f]);

  return (
    <div className="espace-web">
      <div className="barre-onglets" role="tablist" aria-label="Onglets">
        {onglets.map((o) => (
          <div key={o.id} className={"onglet-web" + (o.actif ? " actif" : "")}>
            <button role="tab" aria-selected={o.actif} className="onglet-web-titre" title={o.url} onClick={() => api.webActiver(o.id)}>
              {libelle(o)}
            </button>
            <button className="onglet-web-fermer" aria-label={`Fermer ${libelle(o)}`} onClick={() => api.webFermer(o.id)}>
              <X size={13} aria-hidden />
            </button>
          </div>
        ))}
        <button
          className="onglet-web-nouveau"
          aria-label="Page d'accueil de l'espace web"
          title="Accueil (favoris)"
          onClick={() => api.webAccueil()}
        >
          <House size={15} aria-hidden />
        </button>
      </div>

      <div className="barre-adresse">
        <button className="bouton" disabled={!actif} onClick={() => actif && api.webHistorique(actif.id, -1)} aria-label="Page précédente">
          <ArrowLeft size={16} aria-hidden />
        </button>
        <button className="bouton" disabled={!actif} onClick={() => actif && api.webHistorique(actif.id, 1)} aria-label="Page suivante">
          <ArrowRight size={16} aria-hidden />
        </button>
        <button className="bouton" disabled={!actif} onClick={() => actif && api.webHistorique(actif.id, 0)} aria-label="Recharger">
          <RotateCw size={15} aria-hidden />
        </button>
        <input
          className="champ"
          aria-label="Adresse ou recherche"
          placeholder="Adresse (ex. fr.wikipedia.org) ou recherche"
          value={adresse}
          onChange={(e) => setAdresse(e.target.value)}
          onKeyDown={(e) => e.key === "Enter" && aller()}
        />
        <button className="bouton" disabled={!adresse.trim()} onClick={() => ouvrir(adresse)} title="Ouvrir dans un nouvel onglet">
          <Plus size={15} aria-hidden /> Onglet
        </button>
        <button
          className="bouton"
          disabled={!actif}
          onClick={() => actif && api.ouvrirLienExterne(actif.url)}
          title="Si un site refuse de fonctionner ici (ex. connexion Google), ouvre-le dans ton navigateur habituel."
        >
          <ExternalLink size={15} aria-hidden /> Ouvrir dans mon navigateur
        </button>
        <button className="bouton" onClick={() => setPanneau(!panneau)} aria-pressed={panneau}>
          {panneau ? <PanelRightClose size={15} aria-hidden /> : <PanelRightOpen size={15} aria-hidden />}
          {panneau ? "Masquer le panneau" : "Sources et favoris"}
        </button>
      </div>
      {!enLigne && <p className="message-erreur petit">Hors ligne : les pages web ne peuvent pas se charger.</p>}
      {message && <p className={"petit " + (message.erreur ? "message-erreur" : "texte-doux")}>{message.texte}</p>}

      <div className="corps-web">
        <div ref={zone} className="zone-web">
          {!actif && (
            <div className="accueil-web">
              <h2>Espace web</h2>
              <p className="texte-doux petit">
                Les pages s'ouvrent ici, dans des onglets. Elles n'ont aucun accès à tes notes ni à Révisio ; les
                téléchargements vont dans ton dossier Téléchargements.
              </p>
              <h3>Raccourcis</h3>
              <div className="tuiles-web">
                {RACCOURCIS.map((f) => (
                  <button key={f.url} className="tuile-web" onClick={() => ouvrir(f.url)}>
                    <strong>{f.titre}</strong>
                    <span className="texte-doux petit">{new URL(f.url).hostname}</span>
                  </button>
                ))}
              </div>
              {[...parMatiere.entries()].map(([m, liste]) => (
                <div key={m}>
                  <h3>{m}</h3>
                  <div className="tuiles-web">
                    {liste.map((f) => (
                      <div key={f.id} className="tuile-web">
                        <button className="lien-discret" onClick={() => ouvrir(f.url)}>
                          <strong>{f.titre}</strong>
                        </button>
                        <span className="texte-doux petit">{new URL(f.url).hostname}</span>
                        <button className="lien-discret petit" onClick={() => api.supprimerFavori(f.id).then(chargerListes)}>
                          Retirer
                        </button>
                      </div>
                    ))}
                  </div>
                </div>
              ))}
              {onglets.length > 0 && (
                <p className="texte-doux petit">Tes onglets restent ouverts : clique sur l'un d'eux, en haut, pour y revenir.</p>
              )}
            </div>
          )}
        </div>

        {panneau && (
          <aside className="panneau-web">
            <h3>Enregistrer une source</h3>
            <p className="texte-doux petit">
              Sélectionne un passage dans la page, puis enregistre-le pour un chapitre. Il reste dans Révisio (pas dans
              ton vault) ; l'IA ne s'en sert que si tu coches « inclure mes sources web ».
            </p>
            <button className="bouton bouton-principal" disabled={!actif} onClick={capturer}>
              <TextSelect size={15} aria-hidden /> Enregistrer la sélection
            </button>
            {brouillon && (
              <div className="formulaire-source">
                <label className="petit">
                  Titre
                  <input className="champ" value={brouillon.titre} onChange={(e) => setBrouillon({ ...brouillon, titre: e.target.value })} />
                </label>
                <label className="petit">
                  Chapitre
                  <select
                    value={cle({ matiere: brouillon.matiere, chapitre: brouillon.chapitre })}
                    onChange={(e) => {
                      const c = chapitres.find((x) => cle(x) === e.target.value);
                      if (c) setBrouillon({ ...brouillon, matiere: c.matiere, chapitre: c.chapitre });
                    }}
                  >
                    {matieres.map((m) => (
                      <optgroup key={m} label={m}>
                        {chapitres
                          .filter((c) => c.matiere === m)
                          .map((c) => (
                            <option key={cle(c)} value={cle(c)}>
                              {c.chapitre ?? "Notes générales"}
                            </option>
                          ))}
                      </optgroup>
                    ))}
                  </select>
                </label>
                <textarea
                  className="champ"
                  rows={6}
                  aria-label="Passage à enregistrer"
                  value={brouillon.texte}
                  onChange={(e) => setBrouillon({ ...brouillon, texte: e.target.value })}
                />
                <div className="rangee">
                  <button className="bouton bouton-principal" onClick={enregistrerSource}>
                    Enregistrer
                  </button>
                  <button className="bouton" onClick={() => setBrouillon(null)}>
                    Annuler
                  </button>
                </div>
              </div>
            )}

            <h3>Favoris</h3>
            {nouveauFavori ? (
              <div className="formulaire-source">
                <input
                  className="champ"
                  aria-label="Titre du favori"
                  value={nouveauFavori.titre}
                  onChange={(e) => setNouveauFavori({ ...nouveauFavori, titre: e.target.value })}
                />
                <select
                  aria-label="Matière du favori"
                  value={nouveauFavori.matiere ?? ""}
                  onChange={(e) => setNouveauFavori({ ...nouveauFavori, matiere: e.target.value || null })}
                >
                  <option value="">Général</option>
                  {matieres.map((m) => (
                    <option key={m} value={m}>
                      {m}
                    </option>
                  ))}
                </select>
                <div className="rangee">
                  <button className="bouton bouton-principal" onClick={enregistrerFavori}>
                    Ajouter
                  </button>
                  <button className="bouton" onClick={() => setNouveauFavori(null)}>
                    Annuler
                  </button>
                </div>
              </div>
            ) : (
              <button
                className="bouton"
                disabled={!actif}
                onClick={() => actif && setNouveauFavori({ id: 0, matiere: null, titre: libelle(actif), url: actif.url })}
              >
                <BookmarkPlus size={15} aria-hidden /> Ajouter cette page aux favoris
              </button>
            )}

            <h3>Mes sources web ({sources.length})</h3>
            <ul className="liste-sources">
              {sources.slice(0, 20).map((s) => (
                <li key={s.id}>
                  <button className="lien-discret" onClick={() => ouvrir(s.url)} title={s.url}>
                    {s.titre || s.url}
                  </button>
                  <span className="texte-doux petit">
                    {" "}
                    · {s.matiere}
                    {s.chapitre ? ` › ${s.chapitre}` : ""}
                  </span>
                  <p className="petit extrait-source-web">{s.texte.slice(0, 180)}{s.texte.length > 180 ? "…" : ""}</p>
                  <button className="lien-discret petit" onClick={() => api.supprimerSourceWeb(s.id).then(chargerListes)}>
                    Supprimer
                  </button>
                </li>
              ))}
            </ul>
          </aside>
        )}
      </div>
    </div>
  );
}
