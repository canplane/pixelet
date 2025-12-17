import { useAppContext } from '@/app/providers/app-context';

/** Index page */
function IndexPage() {
  const { store } = useAppContext();

  return (
    <div className="relative flex-1 w-full flex flex-col items-center">
      <main className="flex-1 container h-full" />
    </div>
  );
}

export default IndexPage;
