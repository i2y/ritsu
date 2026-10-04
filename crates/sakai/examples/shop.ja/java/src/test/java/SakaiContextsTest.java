// `sakai build --target archunit --lang ja` が ../../../../通販.ctx から書いた設定。
// 直すときは .ctx を直して書き直す。古くなっていないかは `sakai build --check` が言う。

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
  // 「受注」(ordering) の内側
  private static final DescribedPredicate<JavaClass> SAKAI_ORDERING = resideInAnyPackage("ordering..");
  // 「受注」の公表された言語 shop.ordering.v1 から生成したコード
  private static final DescribedPredicate<JavaClass> SAKAI_ORDERING_PL_SHOP_ORDERING_V1 = resideInAnyPackage("shop.ordering.v1..");
  // 「在庫」(inventory) の内側
  private static final DescribedPredicate<JavaClass> SAKAI_INVENTORY = resideInAnyPackage("inventory..");
  // 「在庫」の公表された言語 warehouse.v1 から生成したコード
  private static final DescribedPredicate<JavaClass> SAKAI_INVENTORY_PL_WAREHOUSE_V1 = resideInAnyPackage("warehouse.v1..");
  // 「配送」(delivery) の内側
  private static final DescribedPredicate<JavaClass> SAKAI_DELIVERY = resideInAnyPackage("delivery..").and(resideOutsideOfPackages("delivery.acl.inventory.."));
  // 「配送」の公表された言語 shop.delivery.v1 から生成したコード
  private static final DescribedPredicate<JavaClass> SAKAI_DELIVERY_PL_SHOP_DELIVERY_V1 = resideInAnyPackage("shop.delivery.v1..");
  // 「配送」の「在庫」に向けた腐敗防止層
  private static final DescribedPredicate<JavaClass> SAKAI_DELIVERY_LAYER_INVENTORY = resideInAnyPackage("delivery.acl.inventory..");
  // 「請求」(billing) の内側
  private static final DescribedPredicate<JavaClass> SAKAI_BILLING = resideInAnyPackage("billing..").and(resideOutsideOfPackages("billing.acl.ordering.."));
  // 「請求」の「受注」に向けた腐敗防止層
  private static final DescribedPredicate<JavaClass> SAKAI_BILLING_LAYER_ORDERING = resideInAnyPackage("billing.acl.ordering..");
  // 「請求」と「配送」の共有カーネル
  private static final DescribedPredicate<JavaClass> SAKAI_BILLING_KERNEL_DELIVERY = resideInAnyPackage("calendars..");
  // 「レビュー」(reviews) の内側
  private static final DescribedPredicate<JavaClass> SAKAI_REVIEWS = resideInAnyPackage("reviews..");

  @ArchTest
  static final ArchRule sakai_ordering =
      noClasses().that(DescribedPredicate.not(SAKAI_ORDERING.or(SAKAI_ORDERING_PL_SHOP_ORDERING_V1)))
          .should().dependOnClassesThat(SAKAI_ORDERING)
          .as("「受注」(ordering) の内側は、「受注」だけが使う");

  @ArchTest
  static final ArchRule sakai_ordering_pl_shop_ordering_v1 =
      noClasses().that(DescribedPredicate.not(SAKAI_ORDERING.or(SAKAI_ORDERING_PL_SHOP_ORDERING_V1).or(SAKAI_DELIVERY).or(SAKAI_DELIVERY_LAYER_INVENTORY).or(SAKAI_BILLING_LAYER_ORDERING)))
          .should().dependOnClassesThat(SAKAI_ORDERING_PL_SHOP_ORDERING_V1)
          .as("「受注」の公表された言語 shop.ordering.v1 から生成したコードは、「受注」、「配送」(delivery) の内側、「配送」の「在庫」に向けた腐敗防止層、「請求」の「受注」に向けた腐敗防止層だけが使う");

  @ArchTest
  static final ArchRule sakai_inventory =
      noClasses().that(DescribedPredicate.not(SAKAI_INVENTORY.or(SAKAI_INVENTORY_PL_WAREHOUSE_V1)))
          .should().dependOnClassesThat(SAKAI_INVENTORY)
          .as("「在庫」(inventory) の内側は、「在庫」だけが使う");

  @ArchTest
  static final ArchRule sakai_inventory_pl_warehouse_v1 =
      noClasses().that(DescribedPredicate.not(SAKAI_ORDERING.or(SAKAI_ORDERING_PL_SHOP_ORDERING_V1).or(SAKAI_INVENTORY).or(SAKAI_INVENTORY_PL_WAREHOUSE_V1).or(SAKAI_DELIVERY_LAYER_INVENTORY)))
          .should().dependOnClassesThat(SAKAI_INVENTORY_PL_WAREHOUSE_V1)
          .as("「在庫」の公表された言語 warehouse.v1 から生成したコードは、「受注」、「在庫」、「配送」の「在庫」に向けた腐敗防止層だけが使う");

  @ArchTest
  static final ArchRule sakai_delivery =
      noClasses().that(DescribedPredicate.not(SAKAI_DELIVERY.or(SAKAI_DELIVERY_PL_SHOP_DELIVERY_V1).or(SAKAI_DELIVERY_LAYER_INVENTORY)))
          .should().dependOnClassesThat(SAKAI_DELIVERY)
          .as("「配送」(delivery) の内側は、「配送」だけが使う");

  @ArchTest
  static final ArchRule sakai_delivery_pl_shop_delivery_v1 =
      noClasses().that(DescribedPredicate.not(SAKAI_ORDERING.or(SAKAI_DELIVERY).or(SAKAI_DELIVERY_PL_SHOP_DELIVERY_V1).or(SAKAI_DELIVERY_LAYER_INVENTORY).or(SAKAI_BILLING).or(SAKAI_BILLING_LAYER_ORDERING)))
          .should().dependOnClassesThat(SAKAI_DELIVERY_PL_SHOP_DELIVERY_V1)
          .as("「配送」の公表された言語 shop.delivery.v1 から生成したコードは、「受注」(ordering) の内側、「配送」、「請求」だけが使う");

  @ArchTest
  static final ArchRule sakai_delivery_layer_inventory =
      noClasses().that(DescribedPredicate.not(SAKAI_DELIVERY.or(SAKAI_DELIVERY_PL_SHOP_DELIVERY_V1).or(SAKAI_DELIVERY_LAYER_INVENTORY)))
          .should().dependOnClassesThat(SAKAI_DELIVERY_LAYER_INVENTORY)
          .as("「配送」の「在庫」に向けた腐敗防止層は、「配送」だけが使う");

  @ArchTest
  static final ArchRule sakai_billing =
      noClasses().that(DescribedPredicate.not(SAKAI_BILLING.or(SAKAI_BILLING_LAYER_ORDERING)))
          .should().dependOnClassesThat(SAKAI_BILLING)
          .as("「請求」(billing) の内側は、「請求」だけが使う");

  @ArchTest
  static final ArchRule sakai_billing_layer_ordering =
      noClasses().that(DescribedPredicate.not(SAKAI_BILLING.or(SAKAI_BILLING_LAYER_ORDERING)))
          .should().dependOnClassesThat(SAKAI_BILLING_LAYER_ORDERING)
          .as("「請求」の「受注」に向けた腐敗防止層は、「請求」だけが使う");

  @ArchTest
  static final ArchRule sakai_billing_kernel_delivery =
      noClasses().that(DescribedPredicate.not(SAKAI_DELIVERY.or(SAKAI_DELIVERY_PL_SHOP_DELIVERY_V1).or(SAKAI_DELIVERY_LAYER_INVENTORY).or(SAKAI_BILLING).or(SAKAI_BILLING_LAYER_ORDERING).or(SAKAI_BILLING_KERNEL_DELIVERY)))
          .should().dependOnClassesThat(SAKAI_BILLING_KERNEL_DELIVERY)
          .as("「請求」と「配送」の共有カーネルは、「配送」、「請求」だけが使う");

  @ArchTest
  static final ArchRule sakai_reviews =
      noClasses().that(DescribedPredicate.not(SAKAI_REVIEWS))
          .should().dependOnClassesThat(SAKAI_REVIEWS)
          .as("「レビュー」(reviews) の内側は、「レビュー」だけが使う");
}
