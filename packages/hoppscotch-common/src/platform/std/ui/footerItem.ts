import { HoppFooterMenuItem } from "../../ui"
import IconGift from "~icons/lucide/gift"
import IconMessageCircleCode from "~icons/lucide/message-circle-code"

export const whatsNew: HoppFooterMenuItem = {
  id: "whats-new",
  text: (t) => t("app.whats_new"),
  icon: IconGift,
  action: {
    type: "link",
    href: "https://docs.postdata.cn/documentation/changelog",
  },
}

export const status: HoppFooterMenuItem = {
  id: "report-issue",
  text: (t) => t("app.report_issue"),
  icon: IconMessageCircleCode,
  action: {
    type: "link",
    href: "https://github.com/mamahuhu-io/postdata/issues/new",
  },
}

export const stdFooterItems = [whatsNew, status]
