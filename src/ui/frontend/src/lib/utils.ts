import { type ClassValue, clsx } from "clsx"
import { twMerge } from "tailwind-merge"
 
export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs))
}

export function isAbsolutePath(path: string): boolean {
  if (!path) return false;
  return path.startsWith("/") || /^[A-Za-z]:[\\/]/.test(path);
}

export function resolveLocalFilePath(root: string, file: string): string {
  if (!file) return "";
  if (isAbsolutePath(file)) return file;
  if (!root) return file;
  const normalizedRoot = root.replace(/[\\/]+$/, "");
  const normalizedFile = file.replace(/^[\\/]+/, "");
  return `${normalizedRoot}/${normalizedFile}`;
}

type MaybeTauri = {
  core?: {
    invoke?: (cmd: string, payload?: Record<string, unknown>) => Promise<unknown>;
    convertFileSrc?: (path: string, protocol?: string) => string;
  };
  tauri?: {
    invoke?: (cmd: string, payload?: Record<string, unknown>) => Promise<unknown>;
    convertFileSrc?: (path: string, protocol?: string) => string;
  };
  event?: {
    listen?: (
      event: string,
      handler: (payload: { event: string; id: number; payload: unknown }) => void
    ) => Promise<(() => void) | { (): void }>;
  };
};

declare global {
  interface Window {
    __TAURI__?: MaybeTauri;
  }
}

export function invokeTauri<T>(cmd: string, payload?: Record<string, unknown>): Promise<T> {
  const coreInvoke = window.__TAURI__?.core?.invoke;
  if (typeof coreInvoke === "function") {
    return coreInvoke(cmd, payload) as Promise<T>;
  }
  const tauriInvoke = window.__TAURI__?.tauri?.invoke;
  if (typeof tauriInvoke === "function") {
    return tauriInvoke(cmd, payload) as Promise<T>;
  }
  return Promise.reject(new Error("Tauri invoke is not available"));
}

export function toAssetUrl(path: string | null | undefined): string {
  if (!path) return "";
  const normalizedPath = path.trim();
  if (!normalizedPath) return "";

  const convert = window.__TAURI__?.core?.convertFileSrc ?? window.__TAURI__?.tauri?.convertFileSrc;
  if (typeof convert === "function") {
    return convert(normalizedPath);
  }
  if (isAbsolutePath(normalizedPath)) {
    const filePath = normalizedPath.replace(/\\/g, "/");
    const urlPath = filePath.startsWith("/") ? filePath : `/${filePath}`;
    return `file://${encodeURI(urlPath)}`;
  }
  return normalizedPath;
}

export async function listenTauriEvent<T>(
  event: string,
  handler: (payload: T) => void
): Promise<() => void> {
  const globalListen = window.__TAURI__?.event?.listen;
  if (typeof globalListen === "function") {
    const unlisten = await globalListen(event, (ev) => {
      handler(ev.payload as T);
    });
    return typeof unlisten === "function" ? unlisten : () => {};
  }

  try {
    const mod = await import("@tauri-apps/api/event");
    const unlisten = await mod.listen<T>(event, (ev) => handler(ev.payload));
    return unlisten;
  } catch (error) {
    throw new Error(`listen event failed: ${String(error)}`);
  }
}
