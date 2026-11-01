/** The image's on-screen size: its own pixels, or shrunk so it fits the viewport. */
export const fittedSize = (
  image: { width: number; height: number },
  viewport: { width: number; height: number },
): { width: number; height: number } => {
  const scale = Math.min(
    1,
    viewport.width / image.width,
    viewport.height / image.height,
  );
  return { width: image.width * scale, height: image.height * scale };
};
