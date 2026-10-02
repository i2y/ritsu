// Areas of a few shapes, from the command line.
type Kind = "square" | "circle";

function area(kind: Kind, size: number): number {
  if (kind === "square") {
    return size * size;
  }
  if (kind === "circle") {
    return 3 * size * size;
  }
  throw new Error(kind);
}

function unused(): number {
  return 1;
}

console.log(area(process.argv[2] as Kind, Number(process.argv[3])));
