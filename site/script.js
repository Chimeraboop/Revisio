// Révisio — site vitrine : effets de défilement et petites interactions.
// Tout reste lisible sans JavaScript ; ce fichier ne fait qu'ajouter le mouvement.

document.documentElement.classList.add("js");
document.body.classList.add("js");

const mouvementReduit = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
const borner = (x, min = 0, max = 1) => Math.min(max, Math.max(min, x));

/* ---------- Thème clair / sombre, au choix du visiteur ---------- */
const boutonTheme = document.querySelector(".bascule-theme");
const couleurTheme = document.querySelector('meta[name="theme-color"]');

function poserTheme(sombre) {
  if (sombre) document.documentElement.dataset.theme = "sombre";
  else delete document.documentElement.dataset.theme;
  boutonTheme.setAttribute("aria-pressed", String(sombre));
  couleurTheme.setAttribute("content", sombre ? "#000000" : "#ffffff");
}
poserTheme(document.documentElement.dataset.theme === "sombre");

boutonTheme.addEventListener("click", () => {
  const sombre = document.documentElement.dataset.theme !== "sombre";
  try { localStorage.setItem("revisio-theme", sombre ? "sombre" : "clair"); } catch (e) {}
  if (document.startViewTransition && !mouvementReduit) {
    document.startViewTransition(() => poserTheme(sombre));
  } else {
    poserTheme(sombre);
  }
});

/* ---------- Carte de l'accueil : se retourne une fois, puis au clic ---------- */
const carte = document.querySelector(".carte-vocab");
const retourner = (etat) => {
  const retournee = etat ?? !carte.classList.contains("retournee");
  carte.classList.toggle("retournee", retournee);
  carte.setAttribute("aria-pressed", String(retournee));
};
carte.addEventListener("click", () => retourner());
if (!mouvementReduit) {
  setTimeout(() => retourner(true), 1700);
  setTimeout(() => retourner(false), 4300);
}

// Le paquet suit légèrement la souris (souris seulement, pas le tactile)
const paquet = document.querySelector(".paquet");
if (!mouvementReduit && window.matchMedia("(pointer: fine)").matches) {
  window.addEventListener("pointermove", (e) => {
    const x = e.clientX / window.innerWidth - 0.5;
    const y = e.clientY / window.innerHeight - 0.5;
    paquet.style.setProperty("--px", `${x * 18}px`);
    paquet.style.setProperty("--py", `${y * 18}px`);
    carte.style.setProperty("--ry", `${x * 14}deg`);
    carte.style.setProperty("--rx", `${-y * 10}deg`);
  });
}

/* ---------- Fonctionnement : la fenêtre change avec l'étape lue ---------- */
const etapes = [...document.querySelectorAll(".etape")];
const ecrans = [...document.querySelectorAll(".ecran")];
const liensMenu = [...document.querySelectorAll(".menu-lien")];
let comptesFaits = false;

function animerComptes() {
  if (comptesFaits) return;
  comptesFaits = true;
  document.querySelectorAll("[data-compte]").forEach((el) => {
    const cible = Number(el.dataset.compte);
    if (mouvementReduit) { el.textContent = cible; return; }
    const debut = performance.now();
    const duree = 1200;
    const pas = (t) => {
      const p = borner((t - debut) / duree);
      el.textContent = Math.round(cible * (1 - Math.pow(1 - p, 4)));
      if (p < 1) requestAnimationFrame(pas);
    };
    requestAnimationFrame(pas);
  });
}

function montrerEtape(n) {
  etapes.forEach((e, i) => e.classList.toggle("actif", i === n));
  ecrans.forEach((e, i) => e.classList.toggle("actif", i === n));
  liensMenu.forEach((l) => l.classList.toggle("actif", l.dataset.pour === String(n)));
  if (n === 1) animerComptes();
}
montrerEtape(0);

const observateurEtapes = new IntersectionObserver(
  (entrees) => {
    entrees.forEach((e) => {
      if (e.isIntersecting) montrerEtape(Number(e.target.dataset.ecran));
    });
  },
  // Sur téléphone, la fenêtre reste collée en haut : on lit l'étape plus bas dans l'écran
  { rootMargin: window.matchMedia("(max-width: 980px)").matches ? "-74% 0px -24% 0px" : "-48% 0px -48% 0px" }
);
etapes.forEach((e) => observateurEtapes.observe(e));

/* ---------- Planning : les étiquettes se posent quand le calendrier arrive ---------- */
const calendrier = document.querySelector(".calendrier");
calendrier.querySelectorAll(".cal-grille .tag").forEach((t, i) => t.style.setProperty("--i", i));
new IntersectionObserver(
  (entrees, obs) => {
    if (entrees[0].isIntersecting) {
      calendrier.classList.add("visible");
      obs.disconnect();
    }
  },
  { threshold: 0.35 }
).observe(calendrier);

/* ---------- IA : liste dépliable (une seule ouverte) ---------- */
const boutonsCreations = [...document.querySelectorAll(".creations button")];
boutonsCreations.forEach((b) =>
  b.addEventListener("click", () => {
    const ouvrir = b.getAttribute("aria-expanded") !== "true";
    boutonsCreations.forEach((autre) => autre.setAttribute("aria-expanded", "false"));
    b.setAttribute("aria-expanded", String(ouvrir));
  })
);

/* ---------- IA : interrupteur En ligne / Local ---------- */
const interrupteur = document.querySelector(".interrupteur");
interrupteur.querySelectorAll("button").forEach((b) =>
  b.addEventListener("click", () => {
    const moteur = b.dataset.moteur;
    interrupteur.classList.toggle("local", moteur === "local");
    interrupteur.querySelectorAll("button").forEach((x) => {
      x.classList.toggle("actif", x === b);
      x.setAttribute("aria-pressed", String(x === b));
    });
    document.querySelectorAll(".moteur-vue").forEach((v) =>
      v.classList.toggle("actif", v.dataset.moteur === moteur)
    );
  })
);

/* ---------- Confidentialité : les mots s'allument à la lecture ---------- */
const declaration = document.querySelector("[data-mots]");
const mots = declaration.textContent.trim().split(/\s+/);
declaration.innerHTML = mots.map((m) => `<span class="mot-lu">${m}</span>`).join(" ");
const spansMots = [...declaration.querySelectorAll(".mot-lu")];

/* ---------- Défilement : une seule boucle pour tous les effets ---------- */
const barre = document.querySelector(".barre");
const sectionTelecharger = document.querySelector(".telecharger");
const scenePhotos = document.querySelector(".photos-scene");
const logoChrome = document.querySelector(".logo-chrome");
let enAttente = false;

function auDefilement() {
  enAttente = false;
  const h = window.innerHeight;
  const y = window.scrollY;
  const total = document.documentElement.scrollHeight - h;

  document.documentElement.style.setProperty("--lecture", total > 0 ? y / total : 0);
  barre.classList.toggle("defilee", y > 24);

  // Photo de cours qui glisse vers la note créée
  if (!mouvementReduit) {
    const r = scenePhotos.getBoundingClientRect();
    scenePhotos.style.setProperty("--p", borner((h - r.top) / (h * 0.75)));
  }

  // Mots de la déclaration
  if (!mouvementReduit) {
    const r = declaration.getBoundingClientRect();
    const p = borner((h * 0.85 - r.top) / (r.height + h * 0.3));
    const allumes = Math.round(p * spansMots.length);
    spansMots.forEach((s, i) => s.classList.toggle("lu", i < allumes));
  }

  // Logo du téléchargement : se redresse et reçoit un reflet
  if (!mouvementReduit) {
    const r = sectionTelecharger.getBoundingClientRect();
    const p = borner((h - r.top) / (h * 0.9));
    logoChrome.style.setProperty("--ly", `${(1 - p) * -32}deg`);
    logoChrome.style.setProperty("--lx", `${(1 - p) * 14}deg`);
    logoChrome.style.setProperty("--ls", 0.82 + p * 0.18);
    logoChrome.style.setProperty("--reflet", `${-60 + p * 120}%`);
  }
}

window.addEventListener("scroll", () => {
  if (!enAttente) {
    enAttente = true;
    requestAnimationFrame(auDefilement);
  }
}, { passive: true });
window.addEventListener("resize", auDefilement);
auDefilement();
