fn main() {
    // Code du client Google (synchronisation avec le téléphone) : jamais dans le dépôt. Il est lu dans le
    // fichier local `google-client.secret` (ignoré par git) ou dans la variable d'environnement
    // REVISIO_GOOGLE_SECRET (compilation sur GitHub Actions), puis intégré au programme.
    println!("cargo:rerun-if-changed=google-client.secret");
    println!("cargo:rerun-if-env-changed=REVISIO_GOOGLE_SECRET");
    let secret = std::fs::read_to_string("google-client.secret")
        .ok()
        .or_else(|| std::env::var("REVISIO_GOOGLE_SECRET").ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    if let Some(s) = secret {
        println!("cargo:rustc-env=REVISIO_GOOGLE_SECRET={s}");
    }
    tauri_build::build()
}
