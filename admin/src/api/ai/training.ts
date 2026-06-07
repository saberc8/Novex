import { api } from "@/lib/api";
import type { TrainingLearningQuery, TrainingLearningRecordsResp } from "@/types/ai-training";

const TRAINING_LEARNING_URL = "/ai/training/learning-records";

export function listTrainingLearningRecords(query: TrainingLearningQuery = {}) {
  return api.get<TrainingLearningRecordsResp>(TRAINING_LEARNING_URL, { ...query });
}
