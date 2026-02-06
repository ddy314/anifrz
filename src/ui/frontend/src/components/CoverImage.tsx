import { useEffect, useMemo, useRef, useState } from "react";
import { api } from "../lib/api";
import { toAssetUrl } from "../lib/utils";

export interface CoverImageMeta {
  width: number;
  height: number;
  isPortrait: boolean;
  isLandscape: boolean;
}

interface CoverImageProps {
  path: string | null | undefined;
  alt: string;
  fallback: string;
  className?: string;
  loading?: "eager" | "lazy";
  sourceMode?: "auto" | "data-url";
  onImageLoad?: (meta: CoverImageMeta) => void;
}

const coverDataUrlCache = new Map<string, string>();

export const CoverImage = ({
  path,
  alt,
  fallback,
  className,
  loading = "eager",
  sourceMode = "auto",
  onImageLoad,
}: CoverImageProps) => {
  const triedDataUrlRef = useRef(false);

  const primarySrc = useMemo(() => {
    const input = path?.trim();
    if (sourceMode === "data-url" && input) {
      const cached = coverDataUrlCache.get(input);
      if (cached) return cached;
    }
    return toAssetUrl(path) || fallback;
  }, [fallback, path, sourceMode]);

  const [src, setSrc] = useState(primarySrc);

  useEffect(() => {
    triedDataUrlRef.current = false;
    setSrc(primarySrc);
  }, [primarySrc]);

  useEffect(() => {
    if (sourceMode !== "data-url") return;
    const input = path?.trim();
    if (!input || input.startsWith("data:")) return;

    const cached = coverDataUrlCache.get(input);
    if (cached) {
      setSrc(cached);
      return;
    }

    let cancelled = false;
    api
      .getCoverDataUrl(input)
      .then((dataUrl) => {
        if (cancelled) return;
        if (!dataUrl) {
          setSrc(fallback);
          return;
        }
        coverDataUrlCache.set(input, dataUrl);
        setSrc(dataUrl);
      })
      .catch(() => {
        if (!cancelled) setSrc(fallback);
      });

    return () => {
      cancelled = true;
    };
  }, [fallback, path, sourceMode]);

  const onError = () => {
    if (triedDataUrlRef.current) {
      if (src !== fallback) setSrc(fallback);
      return;
    }
    triedDataUrlRef.current = true;

    const input = path?.trim();
    if (!input || input.startsWith("data:")) {
      if (src !== fallback) setSrc(fallback);
      return;
    }

    const cached = coverDataUrlCache.get(input);
    if (cached) {
      setSrc(cached);
      return;
    }

    api
      .getCoverDataUrl(input)
      .then((dataUrl) => {
        if (!dataUrl) {
          setSrc(fallback);
          return;
        }
        coverDataUrlCache.set(input, dataUrl);
        setSrc(dataUrl);
      })
      .catch(() => {
        setSrc(fallback);
      });
  };

  const onLoad = (event: React.SyntheticEvent<HTMLImageElement>) => {
    if (!onImageLoad) return;
    const target = event.currentTarget;
    const width = target.naturalWidth || 0;
    const height = target.naturalHeight || 0;
    onImageLoad({
      width,
      height,
      isPortrait: height > width,
      isLandscape: width >= height,
    });
  };

  return (
    <img
      src={src}
      alt={alt}
      className={className}
      loading={loading}
      decoding="async"
      onError={onError}
      onLoad={onLoad}
    />
  );
};
