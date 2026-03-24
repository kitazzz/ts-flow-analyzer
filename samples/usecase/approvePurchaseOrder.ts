import { createResolver, t } from "@tailor-platform/sdk";
import { createContext } from "@tailor-platform/erp-kit/app";
import { getDB } from "@/generated/kysely-tailordb";
import { purchaseCommands } from "@/modules";

export default createResolver({
  name: "approvePurchaseOrder",
  operation: "mutation",
  input: {
    id: t.uuid(),
  },
  body: async (context) => {
    const db = getDB("main-db");
    return db.transaction().execute(async (trx) => {
      const result = await purchaseCommands.approvePurchaseOrder(
        trx,
        { id: context.input.id },
        createContext(context),
      );
      if (!result.ok) {
        switch (result.error.code) {
          case "PURCHASE_PURCHASE_ORDER_NOT_FOUND":
            throw new Error("Purchase order not found");
          case "PURCHASE_PURCHASE_ORDER_NOT_SUBMITTED":
            throw new Error("Purchase order is not in submitted state");
          case "PURCHASE_SUPPLIER_NOT_ACTIVE":
            throw new Error("Supplier is not active");
          case "PURCHASE_PARTNER_NOT_SUPPLIER":
            throw new Error("Partner is not a supplier");
          case "PURCHASE_ITEM_NOT_ACTIVE":
            throw new Error("Item is not active");
          case "PURCHASE_MISSING_COMMERCIAL_SNAPSHOT":
            throw new Error("Missing commercial snapshot");
          case "INSUFFICIENT_PERMISSION":
            throw new Error("Insufficient permission");
        }
      }
      return { id: result.value.purchaseOrder.id };
    });
  },
  output: t.object({ id: t.uuid() }).description("ApprovePurchaseOrder response"),
});
