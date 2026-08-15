export async function completeAdminBatch(tasks: Array<() => Promise<unknown>>): Promise<void> {
  const results = await Promise.allSettled(tasks.map((task) => task()));
  const failures = results.filter((result): result is PromiseRejectedResult => result.status === "rejected");
  if (!failures.length) return;
  const completed = results.length - failures.length;
  const reason = failures[0]?.reason;
  const detail = reason instanceof Error ? reason.message : "请求失败";
  throw new Error(`${completed} 项成功，${failures.length} 项失败：${detail}`);
}
