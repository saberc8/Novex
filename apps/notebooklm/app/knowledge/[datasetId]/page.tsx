import { ChatWorkbenchPage } from "../../workbench";

export default async function Page({ params }: { params: Promise<{ datasetId: string }> }) {
  const { datasetId } = await params;

  return <ChatWorkbenchPage activeRoute="knowledge" initialDatasetId={parseDatasetId(datasetId)} />;
}

function parseDatasetId(value: string) {
  const id = Number(value);
  return Number.isSafeInteger(id) && id > 0 ? id : null;
}
