/**
 * Represents which side of a diff (old/deletions or new/additions).
 * Side of a change, used when mapping review annotations.
 * but allows us to decouple from that library.
 */
export enum DiffSide {
  Old = 0,
  New = 1,
}
