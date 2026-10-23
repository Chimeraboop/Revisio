//! Fichiers audio WAV (PCM) : lecture des morceaux renvoyés par le TTS de Gemini et assemblage
//! en un seul fichier. Analogie Python : le module `wave` de la bibliothèque standard.

#[derive(Debug, Clone, PartialEq)]
pub struct Pcm {
    pub frequence: u32,
    pub canaux: u16,
    pub bits: u16,
    /// Échantillons bruts (little-endian).
    pub donnees: Vec<u8>,
}

impl Pcm {
    pub fn duree_ms(&self) -> u64 {
        let par_seconde = self.frequence as u64 * self.canaux as u64 * (self.bits as u64 / 8);
        if par_seconde == 0 {
            return 0;
        }
        self.donnees.len() as u64 * 1000 / par_seconde
    }

    /// Ajoute un autre morceau au même format.
    pub fn ajouter(&mut self, autre: &Pcm) -> Result<(), String> {
        if (self.frequence, self.canaux, self.bits) != (autre.frequence, autre.canaux, autre.bits) {
            return Err("morceaux audio de formats différents".into());
        }
        self.donnees.extend_from_slice(&autre.donnees);
        Ok(())
    }

    /// Fichier WAV complet (en-tête RIFF de 44 octets + données).
    pub fn en_wav(&self) -> Vec<u8> {
        let octets_par_echantillon = self.bits / 8;
        let debit = self.frequence * self.canaux as u32 * octets_par_echantillon as u32;
        let mut w = Vec::with_capacity(44 + self.donnees.len());
        w.extend_from_slice(b"RIFF");
        w.extend_from_slice(&(36 + self.donnees.len() as u32).to_le_bytes());
        w.extend_from_slice(b"WAVEfmt ");
        w.extend_from_slice(&16u32.to_le_bytes());
        w.extend_from_slice(&1u16.to_le_bytes()); // PCM
        w.extend_from_slice(&self.canaux.to_le_bytes());
        w.extend_from_slice(&self.frequence.to_le_bytes());
        w.extend_from_slice(&debit.to_le_bytes());
        w.extend_from_slice(&(self.canaux * octets_par_echantillon).to_le_bytes());
        w.extend_from_slice(&self.bits.to_le_bytes());
        w.extend_from_slice(b"data");
        w.extend_from_slice(&(self.donnees.len() as u32).to_le_bytes());
        w.extend_from_slice(&self.donnees);
        w
    }
}

fn u16_le(b: &[u8], i: usize) -> Option<u16> {
    Some(u16::from_le_bytes(b.get(i..i + 2)?.try_into().ok()?))
}

fn u32_le(b: &[u8], i: usize) -> Option<u32> {
    Some(u32::from_le_bytes(b.get(i..i + 4)?.try_into().ok()?))
}

/// Lit un fichier WAV PCM (parcourt les blocs « fmt » et « data », quel que soit leur ordre).
pub fn lire_wav(octets: &[u8]) -> Result<Pcm, String> {
    if octets.len() < 12 || &octets[0..4] != b"RIFF" || &octets[8..12] != b"WAVE" {
        return Err("ce n'est pas un fichier WAV".into());
    }
    let mut format = None;
    let mut i = 12;
    while i + 8 <= octets.len() {
        let id = &octets[i..i + 4];
        let taille = u32_le(octets, i + 4).ok_or("WAV tronqué")? as usize;
        let debut = i + 8;
        match id {
            b"fmt " => {
                format = Some((
                    u16_le(octets, debut + 2).ok_or("WAV tronqué")?,
                    u32_le(octets, debut + 4).ok_or("WAV tronqué")?,
                    u16_le(octets, debut + 14).ok_or("WAV tronqué")?,
                ));
            }
            b"data" => {
                let (canaux, frequence, bits) = format.ok_or("bloc « fmt » manquant")?;
                // Certains flux mettent une taille indéfinie (0xFFFFFFFF) : on prend tout ce qui reste.
                let fin = debut.saturating_add(taille).min(octets.len());
                return Ok(Pcm { frequence, canaux, bits, donnees: octets[debut..fin].to_vec() });
            }
            _ => {}
        }
        i = debut + taille + (taille % 2);
    }
    Err("bloc « data » introuvable".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pcm(n: usize) -> Pcm {
        Pcm { frequence: 24_000, canaux: 1, bits: 16, donnees: vec![1; n] }
    }

    #[test]
    fn aller_retour_et_assemblage() {
        let a = pcm(48_000); // 1 s
        let wav = a.en_wav();
        assert_eq!(wav.len(), 44 + 48_000);
        let relu = lire_wav(&wav).unwrap();
        assert_eq!(relu, a);
        assert_eq!(relu.duree_ms(), 1000);

        let mut total = relu.clone();
        total.ajouter(&pcm(24_000)).unwrap();
        assert_eq!(total.duree_ms(), 1500);
        assert!(total.ajouter(&Pcm { frequence: 16_000, ..pcm(2) }).is_err());
    }

    #[test]
    fn bloc_supplementaire_et_erreurs() {
        // Un bloc « LIST » entre « fmt » et « data » est ignoré.
        let wav = pcm(4).en_wav();
        let mut avec_liste = wav[..36].to_vec();
        avec_liste.extend_from_slice(b"LIST");
        avec_liste.extend_from_slice(&3u32.to_le_bytes());
        avec_liste.extend_from_slice(&[0, 0, 0, 0]); // 3 octets + 1 de bourrage
        avec_liste.extend_from_slice(&wav[36..]);
        assert_eq!(lire_wav(&avec_liste).unwrap().donnees, vec![1; 4]);
        assert!(lire_wav(b"pas un wav").is_err());
        assert!(lire_wav(&wav[..30]).is_err());
    }
}
