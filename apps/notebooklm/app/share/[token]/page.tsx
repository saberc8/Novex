import { PublicShareClient } from "@/public-share-client";

export default async function Page({ params }: { params: Promise<{ token: string }> }) {
  const { token } = await params;

  return <PublicShareClient token={token} />;
}
