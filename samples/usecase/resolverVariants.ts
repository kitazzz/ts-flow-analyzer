import { createResolver } from "@tailor-platform/api";

// const binding with name property
const getOrder = createResolver({
  name: "getOrder",
  body: async (ctx) => {
    const order = await ctx.db.order.findUnique({
      where: { id: ctx.input.id },
    });
    if (!order) {
      throw new Error("Order not found");
    }
    return order;
  },
});

// export const binding with name property
export const deleteOrder = createResolver({
  name: "deleteOrder",
  body: async (ctx) => {
    if (!ctx.input.id) {
      throw new Error("ID is required");
    }
    await ctx.db.order.delete({ where: { id: ctx.input.id } });
    return { success: true };
  },
});

// Name fallback to variable name (no name property)
const listOrders = createResolver({
  body: async (ctx) => {
    return await ctx.db.order.findMany();
  },
});
