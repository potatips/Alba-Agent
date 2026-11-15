import { EmbedWidget } from "@/components/teammates/embed-widget";

export function generateStaticParams() {
  return [{ workflowId: "preview" }];
}

export default async function EmbedPage({ params }: { params: Promise<{ workflowId: string }> }) {
  const { workflowId } = await params;
  return <EmbedWidget workflowId={workflowId} />;
}
