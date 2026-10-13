import { createRouter } from '@tanstack/react-router';
import { routeTree } from '@web/routeTree.gen';
import { routeLoadOptions } from '@/shared/components/RouteLoadState';
import { publicRouterBasepath } from './publicBasepath';

export const router = createRouter({
  routeTree,
  basepath: publicRouterBasepath(),
  ...routeLoadOptions,
});

declare module '@tanstack/react-router' {
  interface Register {
    router: typeof router;
  }
}
