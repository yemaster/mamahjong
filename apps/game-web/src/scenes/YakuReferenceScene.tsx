import { useSceneReady } from "../components/SceneTransition";
import { YakuReferencePage } from "../game/YakuReference";
import { navigateTo } from "../routing";

export default function YakuReferenceScene() {
  useSceneReady(true);

  return (
    <YakuReferencePage onBack={() => navigateTo({ kind: "lobby" })} />
  );
}
