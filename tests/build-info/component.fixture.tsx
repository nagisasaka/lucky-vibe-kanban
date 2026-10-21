import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { createInstance } from "i18next";
import { I18nextProvider } from "react-i18next";
import { BuildInfoLabel } from "../../packages/ui/src/components/BuildInfoLabel";
import en from "../../packages/web-core/src/i18n/locales/en/common.json";
import ja from "../../packages/web-core/src/i18n/locales/ja/common.json";
import type { BuildInfo } from "../../shared/types";

export async function render(buildInfo?: BuildInfo | null, lng = "ja") {
  const i18n = createInstance();
  await i18n.init({
    lng,
    resources: { en: { common: en }, ja: { common: ja } },
  });
  return renderToStaticMarkup(
    createElement(
      I18nextProvider,
      { i18n },
      createElement(BuildInfoLabel, { buildInfo }),
    ),
  );
}
