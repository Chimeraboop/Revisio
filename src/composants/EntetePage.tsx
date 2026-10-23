// En-tête commun des écrans : icône, petit sur-titre, titre, description et actions à droite.
import type { LucideIcon } from "lucide-react";
import type { ReactNode } from "react";

interface Props {
  icone: LucideIcon;
  titre: ReactNode;
  surTitre?: string;
  description?: ReactNode;
  actions?: ReactNode;
}

export default function EntetePage({ icone: Icone, titre, surTitre, description, actions }: Props) {
  return (
    <header className="entete-page apparait">
      <div className="entete-page-icone" aria-hidden>
        <Icone size={22} strokeWidth={1.8} />
      </div>
      <div className="entete-page-texte">
        {surTitre && <p className="sur-titre">{surTitre}</p>}
        <h1>{titre}</h1>
        {description && <p className="texte-doux">{description}</p>}
      </div>
      {actions && <div className="entete-page-actions">{actions}</div>}
    </header>
  );
}
