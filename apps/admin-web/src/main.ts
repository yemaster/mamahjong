import { createApp } from "vue";
import PrimeVue from "primevue/config";
import ConfirmationService from "primevue/confirmationservice";
import ToastService from "primevue/toastservice";
import "primeicons/primeicons.css";
import "primeflex/primeflex.css";
import App from "./App.vue";
import { router } from "./router";
import { AdminTheme } from "./theme";
import "./styles.css";

createApp(App)
  .use(PrimeVue, {
    locale: {
      accept: "确定",
      reject: "取消",
      emptyMessage: "暂无数据",
      emptySearchMessage: "未找到结果",
      rowsPerPageLabel: "每页",
    },
    theme: {
      preset: AdminTheme,
      options: {
        darkModeSelector: false,
        cssLayer: false,
      },
    },
    ripple: true,
  })
  .use(ConfirmationService)
  .use(ToastService)
  .use(router)
  .mount("#app");
