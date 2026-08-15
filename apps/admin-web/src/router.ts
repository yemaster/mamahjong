import { createRouter, createWebHistory } from "vue-router";

export const router = createRouter({
  history: createWebHistory("/admin/"),
  routes: [
    {
      path: "/login",
      name: "login",
      component: () => import("./pages/LoginPage.vue"),
    },
    {
      path: "/",
      component: () => import("./components/AdminLayout.vue"),
      children: [
        { path: "", name: "overview", component: () => import("./pages/OverviewPage.vue") },
        { path: "users", name: "users", component: () => import("./pages/UsersPage.vue") },
        { path: "rooms", name: "rooms", component: () => import("./pages/RoomsPage.vue") },
        { path: "matches", name: "matches", component: () => import("./pages/MatchesPage.vue") },
        { path: "matches/:matchId", name: "match-detail", component: () => import("./pages/MatchDetailPage.vue"), props: true },
        { path: "characters", name: "characters", component: () => import("./pages/CharactersPage.vue") },
        { path: "assets", name: "assets", component: () => import("./pages/AssetsPage.vue") },
        { path: "characters/new", name: "character-new", component: () => import("./pages/CharacterFormPage.vue") },
        { path: "characters/:characterId/edit", name: "character-edit", component: () => import("./pages/CharacterFormPage.vue"), props: true },
        { path: "tablecloths", name: "tablecloths", component: () => import("./pages/TableclothsPage.vue") },
        { path: "tablecloths/new", name: "tablecloth-new", component: () => import("./pages/TableclothFormPage.vue") },
        { path: "tablecloths/:tableclothId/edit", name: "tablecloth-edit", component: () => import("./pages/TableclothFormPage.vue"), props: true },
        { path: "music", name: "music", component: () => import("./pages/MusicPage.vue") },
        { path: "music/new", name: "music-new", component: () => import("./pages/MusicFormPage.vue") },
        { path: "music/:musicId/edit", name: "music-edit", component: () => import("./pages/MusicFormPage.vue"), props: true },
        { path: "database", name: "database", component: () => import("./pages/DatabasePage.vue") },
        { path: "audit", name: "audit", component: () => import("./pages/AuditPage.vue") },
      ],
    },
    { path: "/:pathMatch(.*)*", redirect: "/" },
  ],
});
