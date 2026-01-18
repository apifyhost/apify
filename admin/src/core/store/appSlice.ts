import { createSlice, createAsyncThunk, PayloadAction } from '@reduxjs/toolkit';

interface AppState {
  loading: boolean;
  initialized: boolean;
  error: string | null;
  sidebarCollapsed: boolean;
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
  currentUser: null,
};

export const fetchInitialData = createAsyncThunk(
  'app/fetchInitialData',
  async () => {
    // 这里可以加载初始配置数据
    // const response = await axios.get('/apify/admin/config');
    // return response.data;
    
    // 暂时返回模拟数据
    return {
      user: {
        name: 'Admin',
        role: 'administrator',
      },
    };
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
  },
  extraReducers: (builder) => {
    builder
      .addCase(fetchInitialData.pending, (state) => {
        state.loading = true;
        state.error = null;
      })
      .addCase(fetchInitialData.fulfilled, (state, action) => {
        state.loading = false;
        state.initialized = true;
        state.currentUser = action.payload.user;
      })
      .addCase(fetchInitialData.rejected, (state, action) => {
        state.loading = false;
        state.error = action.error.message || 'Failed to initialize';
        state.initialized = true; // 即使失败也标记为已初始化
      });
  },
});

export const { toggleSidebar, setSidebarCollapsed, setCurrentUser } = appSlice.actions;
export default appSlice.reducer;
