export interface ImageCaptchaResp {
  isEnabled: boolean;
  uuid: string;
  img: string;
}

export interface AccountLoginCommand {
  username: string;
  password: string;
  authType: "ACCOUNT";
  clientId: string;
  captcha?: string;
  uuid?: string;
}

export interface LoginResp {
  token: string;
  expire: string;
}
