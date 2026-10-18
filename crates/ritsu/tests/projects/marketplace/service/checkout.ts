// The marketplace's checkout, written by hand on the package ritsu gen writes from this project
// (`ritsu gen --root . --out generated`): the rule splits what the buyer paid, and the book records
// the sale, in one call and with no workflow. What the rule returns is the package's type of its
// unit, units.USDc, which is what the book's sale takes: were the rule to count in whole dollars,
// tsc would stop here.
import { books, rules, units } from "../generated/typescript/index";

/** Record a sale of `paid` for `shop`: the marketplace's fee by the shop's plan, the rest to the shop. */
export async function checkout(book: books.marketplace.Book, order: string, shop: string, plan: rules.sale_split.Plan, paid: units.USDc): Promise<books.marketplace.Result> {
  const split = rules.sale_split.sale_split(plan, paid);
  return book.sale.do({ order, shop, to_shop: split.to_shop, fee: split.fee });
}
