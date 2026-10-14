import type { FormulaV2 } from "../flow/ast";
import type { EntityType, UserColumn, UserViewDefinition } from "../types";

export type ViewPackage = {
  format: "pclm-view-config"; version: 1; includesAllColumnsForSql: boolean;
  views: { name: string; definition: UserViewDefinition }[];
  columns: { entityType: EntityType; fieldName: string; kind: UserColumn["kind"]; choices: string[]; formula?: FormulaV2 | null }[];
};
export type ViewPackagePreview = {
  views: { name: string; importName: string }[];
  columns: { entityType: EntityType; fieldName: string; action: "add" | "reuse" | "conflict" }[];
  errors: string[]; includesAllColumnsForSql: boolean; token: string;
};
export type ViewPackagePick = { name: string; json: string; preview: ViewPackagePreview };
