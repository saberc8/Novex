export interface TrainingLearningQuery {
  scope?: "self" | "tenant";
  userId?: number;
}

export interface TrainingLearningSummaryResp {
  completionRate: number;
  pendingTaskCount: number;
  quizAverageScore: number;
  weakPointCount: number;
}

export interface TrainingLearningTaskResp {
  title: string;
  source: string;
  due: string;
  status: string;
}

export interface TrainingLearningRecordResp {
  id: number;
  kind: string;
  title: string;
  detail: string;
  status: string;
  score?: number | null;
  learnerId: number;
  learnerName: string;
  createTime: string;
}

export interface TrainingWeakPointResp {
  topic: string;
  evidence: string;
  count: number;
  lastSeenAt: string;
}

export interface TrainingLearningRecordsResp {
  scope: "self" | "tenant";
  summary: TrainingLearningSummaryResp;
  tasks: TrainingLearningTaskResp[];
  records: TrainingLearningRecordResp[];
  weakPoints: TrainingWeakPointResp[];
}
