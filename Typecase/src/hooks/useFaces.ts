import { useCallback, useSyncExternalStore } from "react";
import * as faceStore from "../data/face";

/** React binding for the backend-backed face store. Loads the catalog on
    first use and re-renders on refresh. */
export function useFaces() {
  const subscribe = useCallback(faceStore.subscribe, []);
  const loaded = useSyncExternalStore(
    subscribe,
    () => faceStore.catalogMeta().loaded,
    () => false
  );
  const error = useSyncExternalStore(
    subscribe,
    () => faceStore.catalogMeta().error,
    () => null
  );
  const faces = useSyncExternalStore(
    subscribe,
    () => faceStore.allFaces(),
    () => []
  );

  if (!loaded && !error) {
    // fire-and-forget: the store notifies when done
    void faceStore.loadCatalog();
  }

  return { faces, loaded, error, reload: faceStore.loadCatalog };
}
