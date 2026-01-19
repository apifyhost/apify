import { createSlice, createAsyncThunk, PayloadAction } from '@reduxjs/toolkit';

interface AppState {
  loading: boolean;
  initialized: boolean;
  error: string | null;
  sidebarCollapsed: boolean;
  isAuthenticated: boolean;
  apiKey: string | null;
  currentUser: {
    name: string;
    role: string;
  } | null;
}

const initialState: AppState = {
  loading: false,
  initialized: false,
  error: null,
  sidebarCollapsed: false,
  isAuthenticated: !!localStorage.getItem('apiKey'),
  apiKey: localStorage.getItem('apiKey'),
  currentUser: null,
};

export const login = createAsyncThunk(
  'app/login',
  async (apiKey: string, { rejectWithValue }) => {
    try {
      // 验证 API Key
      const response = await fetch('/apify/admin/apis', {
        headers: {
          'X-API-KEY': apiKey,
        },
      });
      
      if (!response.ok) {
        return rejectWithValue('Invalid API Key');
      }
      
      // 保存到 localStorage
      localStorage.setItem('apiKey', apiKey);
      
      return {
        apiKey,
        user: {
          name: 'Admin',
          role: 'administrator',
        },
      };
    } catch (error) {
      return rejectWithValue('Login failed');
    }
  }
);

export const fetchInitialData = createAsyncThunk(
  'app/fetchInitialData',
  async () => {
    // 从 localStorage 恢复认证状态
    const apiKey = localStorage.getItem('apiKey');
    
    if (apiKey) {
      return {
        apiKey,
        user: {
          name: 'Admin',
          role: 'administrator',
        },
      };
    }
    
    return { apiKey: null, user: null };
  }
);

const appSlice = createSlice({
  name: 'app',
  initialState,
  reducers: {
    toggleSidebar: (state) => {
      state.sidebarCollapsed = !state.sidebarCollapsed;
    },
    setSidebarCollapsed: (state, action: PayloadAction<boolean>) => {
      state.sidebarCollapsed = action.payload;
    },
    setCurrentUser: (state, action) => {
      state.currentUser = action.payload;
    },
    logout: (state) => {
      state.isAuthenticated = false;
      state.apiKey = null;
      state.currentUser = null;
      localStorage.removeItem('apiKey');
    },
  },
  extraReducers: (builder) => {
    builder
      .addCase(login.pending, (state) => {
        state.loading = true;
        state.error = null;
      })
      .addCase(login.fulfilled, (state, action) => {
        state.loading = false;
        state.isAuthenticated = true;
        state.apiKey = action.payload.apiKey;
        state.currentUser = action.payload.user;
      })
      .addCase(login.rejected, (state, action) => {
        state.loading = false;
        state.error = action.payload as string;
      })
      .addCase(fetchInitialData.pending, (state) => {
        state.loading = true;
        state.error = null;
      })
      .addCase(fetchInitialData.fulfilled, (state, action) => {
        state.loading = false;
        state.initialized = true;
        if (action.payload.apiKey) {
          state.isAuthenticated = true;
          state.apiKey = action.payload.apiKey;
          state.currentUser = action.payload.user;
        }
      })
      .addCase(fetchInitialData.rejected, (state, action) => {
        state.loading = false;
        state.error = action.error.message || 'Failed to initialize';
        state.initialized = true;
      });
  },
});

export const { toggleSidebar, setSidebarCollapsed, setCurrentUser, logout } = appSlice.actions;
export default appSlice.reducer;
