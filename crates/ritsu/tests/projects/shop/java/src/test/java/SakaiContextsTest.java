// Written by `sakai build --target archunit` from ../../../../shop.ctx.
// Edit the .ctx files and write it again; `sakai build --check` says whether it is up to date.

import static com.tngtech.archunit.core.domain.JavaClass.Predicates.resideInAnyPackage;
import static com.tngtech.archunit.core.domain.JavaClass.Predicates.resideOutsideOfPackages;
import static com.tngtech.archunit.lang.syntax.ArchRuleDefinition.noClasses;

import com.tngtech.archunit.base.DescribedPredicate;
import com.tngtech.archunit.core.domain.JavaClass;
import com.tngtech.archunit.junit.AnalyzeClasses;
import com.tngtech.archunit.junit.ArchTest;
import com.tngtech.archunit.lang.ArchRule;

@AnalyzeClasses(packages = {"billing", "calendars", "delivery", "inventory", "ordering", "reviews", "shop", "warehouse"})
class SakaiContextsTest {
  // The inside of Ordering (ordering)
  private static final DescribedPredicate<JavaClass> SAKAI_ORDERING = resideInAnyPackage("ordering..");
  // The code made from Ordering's published language shop.ordering.v1
  private static final DescribedPredicate<JavaClass> SAKAI_ORDERING_PL_SHOP_ORDERING_V1 = resideInAnyPackage("shop.ordering.v1..");
  // The inside of Inventory (inventory)
  private static final DescribedPredicate<JavaClass> SAKAI_INVENTORY = resideInAnyPackage("inventory..");
  // The code made from Inventory's published language warehouse.v1
  private static final DescribedPredicate<JavaClass> SAKAI_INVENTORY_PL_WAREHOUSE_V1 = resideInAnyPackage("warehouse.v1..");
  // The inside of Delivery (delivery)
  private static final DescribedPredicate<JavaClass> SAKAI_DELIVERY = resideInAnyPackage("delivery..").and(resideOutsideOfPackages("delivery.acl.inventory.."));
  // The code made from Delivery's published language shop.delivery.v1
  private static final DescribedPredicate<JavaClass> SAKAI_DELIVERY_PL_SHOP_DELIVERY_V1 = resideInAnyPackage("shop.delivery.v1..");
  // Delivery's anticorruption layer toward Inventory
  private static final DescribedPredicate<JavaClass> SAKAI_DELIVERY_LAYER_INVENTORY = resideInAnyPackage("delivery.acl.inventory..");
  // The inside of Billing (billing)
  private static final DescribedPredicate<JavaClass> SAKAI_BILLING = resideInAnyPackage("billing..").and(resideOutsideOfPackages("billing.acl.ordering.."));
  // Billing's anticorruption layer toward Ordering
  private static final DescribedPredicate<JavaClass> SAKAI_BILLING_LAYER_ORDERING = resideInAnyPackage("billing.acl.ordering..");
  // The shared kernel of Billing and Delivery
  private static final DescribedPredicate<JavaClass> SAKAI_BILLING_KERNEL_DELIVERY = resideInAnyPackage("calendars..");
  // The inside of Reviews (reviews)
  private static final DescribedPredicate<JavaClass> SAKAI_REVIEWS = resideInAnyPackage("reviews..");

  @ArchTest
  static final ArchRule sakai_ordering =
      noClasses().that(DescribedPredicate.not(SAKAI_ORDERING.or(SAKAI_ORDERING_PL_SHOP_ORDERING_V1)))
          .should().dependOnClassesThat(SAKAI_ORDERING)
          .as("The inside of Ordering (ordering) is used only by Ordering");

  @ArchTest
  static final ArchRule sakai_ordering_pl_shop_ordering_v1 =
      noClasses().that(DescribedPredicate.not(SAKAI_ORDERING.or(SAKAI_ORDERING_PL_SHOP_ORDERING_V1).or(SAKAI_DELIVERY).or(SAKAI_DELIVERY_LAYER_INVENTORY).or(SAKAI_BILLING_LAYER_ORDERING)))
          .should().dependOnClassesThat(SAKAI_ORDERING_PL_SHOP_ORDERING_V1)
          .as("The code made from Ordering's published language shop.ordering.v1 is used only by Ordering, the inside of Delivery (delivery), Delivery's anticorruption layer toward Inventory and Billing's anticorruption layer toward Ordering");

  @ArchTest
  static final ArchRule sakai_inventory =
      noClasses().that(DescribedPredicate.not(SAKAI_INVENTORY.or(SAKAI_INVENTORY_PL_WAREHOUSE_V1)))
          .should().dependOnClassesThat(SAKAI_INVENTORY)
          .as("The inside of Inventory (inventory) is used only by Inventory");

  @ArchTest
  static final ArchRule sakai_inventory_pl_warehouse_v1 =
      noClasses().that(DescribedPredicate.not(SAKAI_ORDERING.or(SAKAI_ORDERING_PL_SHOP_ORDERING_V1).or(SAKAI_INVENTORY).or(SAKAI_INVENTORY_PL_WAREHOUSE_V1).or(SAKAI_DELIVERY_LAYER_INVENTORY)))
          .should().dependOnClassesThat(SAKAI_INVENTORY_PL_WAREHOUSE_V1)
          .as("The code made from Inventory's published language warehouse.v1 is used only by Ordering, Inventory and Delivery's anticorruption layer toward Inventory");

  @ArchTest
  static final ArchRule sakai_delivery =
      noClasses().that(DescribedPredicate.not(SAKAI_DELIVERY.or(SAKAI_DELIVERY_PL_SHOP_DELIVERY_V1).or(SAKAI_DELIVERY_LAYER_INVENTORY)))
          .should().dependOnClassesThat(SAKAI_DELIVERY)
          .as("The inside of Delivery (delivery) is used only by Delivery");

  @ArchTest
  static final ArchRule sakai_delivery_pl_shop_delivery_v1 =
      noClasses().that(DescribedPredicate.not(SAKAI_ORDERING.or(SAKAI_DELIVERY).or(SAKAI_DELIVERY_PL_SHOP_DELIVERY_V1).or(SAKAI_DELIVERY_LAYER_INVENTORY).or(SAKAI_BILLING).or(SAKAI_BILLING_LAYER_ORDERING)))
          .should().dependOnClassesThat(SAKAI_DELIVERY_PL_SHOP_DELIVERY_V1)
          .as("The code made from Delivery's published language shop.delivery.v1 is used only by the inside of Ordering (ordering), Delivery and Billing");

  @ArchTest
  static final ArchRule sakai_delivery_layer_inventory =
      noClasses().that(DescribedPredicate.not(SAKAI_DELIVERY.or(SAKAI_DELIVERY_PL_SHOP_DELIVERY_V1).or(SAKAI_DELIVERY_LAYER_INVENTORY)))
          .should().dependOnClassesThat(SAKAI_DELIVERY_LAYER_INVENTORY)
          .as("Delivery's anticorruption layer toward Inventory is used only by Delivery");

  @ArchTest
  static final ArchRule sakai_billing =
      noClasses().that(DescribedPredicate.not(SAKAI_BILLING.or(SAKAI_BILLING_LAYER_ORDERING)))
          .should().dependOnClassesThat(SAKAI_BILLING)
          .as("The inside of Billing (billing) is used only by Billing");

  @ArchTest
  static final ArchRule sakai_billing_layer_ordering =
      noClasses().that(DescribedPredicate.not(SAKAI_BILLING.or(SAKAI_BILLING_LAYER_ORDERING)))
          .should().dependOnClassesThat(SAKAI_BILLING_LAYER_ORDERING)
          .as("Billing's anticorruption layer toward Ordering is used only by Billing");

  @ArchTest
  static final ArchRule sakai_billing_kernel_delivery =
      noClasses().that(DescribedPredicate.not(SAKAI_DELIVERY.or(SAKAI_DELIVERY_PL_SHOP_DELIVERY_V1).or(SAKAI_DELIVERY_LAYER_INVENTORY).or(SAKAI_BILLING).or(SAKAI_BILLING_LAYER_ORDERING).or(SAKAI_BILLING_KERNEL_DELIVERY)))
          .should().dependOnClassesThat(SAKAI_BILLING_KERNEL_DELIVERY)
          .as("The shared kernel of Billing and Delivery is used only by Delivery and Billing");

  @ArchTest
  static final ArchRule sakai_reviews =
      noClasses().that(DescribedPredicate.not(SAKAI_REVIEWS))
          .should().dependOnClassesThat(SAKAI_REVIEWS)
          .as("The inside of Reviews (reviews) is used only by Reviews");
}
