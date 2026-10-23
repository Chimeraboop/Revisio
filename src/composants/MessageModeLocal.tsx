// Rappel affiché quand le mode local est actif sur un écran qui a besoin de Gemini.
import Message from "./Message";

export default function MessageModeLocal({ quoi }: { quoi: string }) {
  return (
    <Message genre="info">
      <strong>Mode local</strong> : {quoi} demande Gemini. Repasse « En ligne » avec l'interrupteur en haut à droite ; ce
      qui est déjà enregistré reste utilisable.
    </Message>
  );
}
