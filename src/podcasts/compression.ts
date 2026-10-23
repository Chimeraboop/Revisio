// Compression des podcasts : WAV (≈ 2,9 Mo par minute) → Opus dans un conteneur WebM (≈ 0,25 Mo par minute),
// avec l'encodeur intégré à WebView2 (WebCodecs) via Mediabunny, chargé seulement quand il sert.
// Si l'encodeur n'est pas disponible, on garde simplement le WAV.

const FREQUENCE = 24_000; // fréquence de l'audio renvoyé par la synthèse vocale de Gemini
const DEBIT = 32_000; // 32 kbit/s : très bonne qualité pour de la voix

export async function compresserEnOpus(wav: ArrayBuffer): Promise<Uint8Array | null> {
  const mb = await import("mediabunny");
  const qualite = new mb.Quality({ bitrate: DEBIT });
  if (!(await mb.canEncodeAudio("opus", { numberOfChannels: 1, sampleRate: FREQUENCE, quality: qualite }))) return null;
  const contexte = new OfflineAudioContext({ numberOfChannels: 1, length: 1, sampleRate: FREQUENCE });
  const tampon = await contexte.decodeAudioData(wav);
  const sortie = new mb.Output({ format: new mb.WebMOutputFormat(), target: new mb.BufferTarget() });
  const source = new mb.AudioBufferSource({ codec: "opus", quality: qualite });
  sortie.addAudioTrack(source);
  await sortie.start();
  await source.add(tampon);
  await sortie.finalize();
  return sortie.target.buffer ? new Uint8Array(sortie.target.buffer) : null;
}

/** Type MIME d'après le nom du fichier audio enregistré. */
export function typeAudio(fichier: string): string {
  return fichier.endsWith(".webm") ? "audio/webm" : "audio/wav";
}
