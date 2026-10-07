// @pxl/web/app/router.tsx

import { createBrowserRouter, Navigate, Outlet, RouterProvider } from "react-router-dom";
import type { RouteObject } from "react-router-dom";

import { devRoutes } from "@/feature/dev/route.tsx";
import { AppLayout } from "./layout.tsx";
import WorldPage from "@/page/world.tsx";

const routes: RouteObject[] = [
  {
    path: "/",
    element: (
      <AppLayout>
        <Outlet />
      </AppLayout>
    ),
    children: [
      { path: "", element: <WorldPage /> },

      ...(__IS_DEV__ ? devRoutes : []),

      { path: "*", element: <Navigate to="/" replace /> },
    ],
  },
];

const router = createBrowserRouter(routes);

export function AppRouter() {
  return <RouterProvider router={router} />;
}
