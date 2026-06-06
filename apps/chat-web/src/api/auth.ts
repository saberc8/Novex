import { apiRequest } from "@/lib/api";
import type { AccountLoginCommand, ImageCaptchaResp, LoginResp } from "@/types/auth";

export function getImageCaptcha() {
  return apiRequest<ImageCaptchaResp>("/captcha/image");
}

export function accountLogin(data: AccountLoginCommand) {
  return apiRequest<LoginResp>("/auth/login", {
    method: "POST",
    body: data
  });
}
