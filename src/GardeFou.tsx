// Garde-fou : si un écran plante, on affiche l'erreur au lieu d'une fenêtre vide,
// et le reste de l'app reste utilisable.
import { Component, type ReactNode } from "react";
import { RotateCcw, TriangleAlert } from "lucide-react";
import EntetePage from "./composants/EntetePage";

interface Props {
  children: ReactNode;
  /** Change quand on navigue : le garde-fou se réarme. */
  cle?: string;
}

interface Etat {
  erreur: Error | null;
  cle?: string;
}

export default class GardeFou extends Component<Props, Etat> {
  state: Etat = { erreur: null, cle: this.props.cle };

  static getDerivedStateFromError(erreur: Error): Partial<Etat> {
    return { erreur };
  }

  static getDerivedStateFromProps(props: Props, etat: Etat): Partial<Etat> | null {
    return props.cle !== etat.cle ? { erreur: null, cle: props.cle } : null;
  }

  render() {
    if (!this.state.erreur) return this.props.children;
    return (
      <div className="page">
        <EntetePage
          icone={TriangleAlert}
          titre="Oups, cet écran a rencontré un problème"
          description="Le reste de Révisio fonctionne : tu peux changer d'écran dans le menu."
        />
        <pre className="carte message-erreur erreur-detail">
          {this.state.erreur.message}
          {"\n\n"}
          {this.state.erreur.stack?.split("\n").slice(0, 6).join("\n")}
        </pre>
        <button className="bouton" onClick={() => this.setState({ erreur: null })}>
          <RotateCcw size={15} aria-hidden /> Réessayer
        </button>
      </div>
    );
  }
}
