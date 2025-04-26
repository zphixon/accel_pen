import { useLocation, useSearchParams } from 'wouter';
import { Suspense, use, useEffect } from 'react';
import { v4 as uuidv4 } from 'uuid'

import * as api from '../api.tsx';
import * as types from '../../../backend/bindings/index.ts';
import { useLocalStorage } from 'react-use';

interface LoginProps {
  finish: boolean,
}
function Login({ finish }: LoginProps) {
  let [params, _setParams] = useSearchParams();

  if (finish) {
    window.location.href = api.apiUrl().href;
    return null;
  }
}

export default Login