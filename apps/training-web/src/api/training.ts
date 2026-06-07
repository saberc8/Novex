import { apiRequest } from "@/lib/api";
import type { TrainingLearningQuery, TrainingLearningRecordsResp } from "@/types/training";

const TRAINING_LEARNING_URL = "/ai/training/learning-records";

export function listTrainingLearningRecords(query: TrainingLearningQuery = {}) {
  return apiRequest<TrainingLearningRecordsResp>(TRAINING_LEARNING_URL, {
    query
  });
}
