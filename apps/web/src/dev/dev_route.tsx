// @pxl/web/dev/dev_route.tsx

import { Outlet } from "react-router-dom";
import type { RouteObject } from "react-router-dom";

import TestPage from "@/dev/test_page.tsx";

/** Debug mode only accessible routes */
export const devRoutes: RouteObject[] = [
  {
    path: "dev",
    element: <Outlet />,
    children: [{ path: "test", element: <TestPage /> }],
  },
];
