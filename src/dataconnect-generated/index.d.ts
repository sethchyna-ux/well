import { ConnectorConfig, DataConnect, QueryRef, QueryPromise, ExecuteQueryOptions, MutationRef, MutationPromise, DataConnectSettings } from 'firebase/data-connect';

export const connectorConfig: ConnectorConfig;
export const dataConnectSettings: DataConnectSettings;

export type TimestampString = string;
export type UUIDString = string;
export type Int64String = string;
export type DateString = string;




export interface AddCommandHistoryData {
  commandHistory_insert: CommandHistory_Key;
}

export interface AddCommandHistoryVariables {
  cmd: string;
  tag?: string | null;
}

export interface CommandHistory_Key {
  id: UUIDString;
  __typename?: 'CommandHistory_Key';
}

export interface CreateProfileData {
  profile_insert: Profile_Key;
}

export interface CreateProfileVariables {
  name: string;
  metadata?: string | null;
}

export interface CreateShellConfigData {
  shellConfig_insert: ShellConfig_Key;
}

export interface CreateShellConfigVariables {
  profileId: UUIDString;
  prompt: string;
  alias: string;
}

export interface CreateTerminalThemeData {
  terminalTheme_insert: TerminalTheme_Key;
}

export interface CreateTerminalThemeVariables {
  name: string;
  palette: string;
  profileId: UUIDString;
}

export interface CreateUserData {
  user_insert: User_Key;
}

export interface DeleteCommandHistoryData {
  commandHistory_delete?: CommandHistory_Key | null;
}

export interface DeleteCommandHistoryVariables {
  id: UUIDString;
}

export interface DeleteDeviceSyncData {
  deviceSync_delete?: DeviceSync_Key | null;
}

export interface DeleteDeviceSyncVariables {
  id: UUIDString;
}

export interface DeleteProfileData {
  profile_delete?: Profile_Key | null;
}

export interface DeleteProfileVariables {
  id: UUIDString;
}

export interface DeleteShellConfigData {
  shellConfig_delete?: ShellConfig_Key | null;
}

export interface DeleteShellConfigVariables {
  id: UUIDString;
}

export interface DeleteTerminalThemeData {
  terminalTheme_delete?: TerminalTheme_Key | null;
}

export interface DeleteTerminalThemeVariables {
  id: UUIDString;
}

export interface DeleteUserData {
  user_delete?: User_Key | null;
}

export interface DeviceSync_Key {
  id: UUIDString;
  __typename?: 'DeviceSync_Key';
}

export interface GetCommandHistoryData {
  commandHistory?: {
    commandString: string;
    timestamp: TimestampString;
  };
}

export interface GetCommandHistoryVariables {
  id: UUIDString;
}

export interface GetCurrentUserData {
  user?: {
    username: string;
    email: string;
  };
}

export interface GetDeviceSyncData {
  deviceSync?: {
    deviceName: string;
    lastSeen: TimestampString;
  };
}

export interface GetDeviceSyncVariables {
  id: UUIDString;
}

export interface GetProfileData {
  profile?: {
    name: string;
    metadata?: string | null;
  };
}

export interface GetProfileVariables {
  id: UUIDString;
}

export interface GetShellConfigData {
  shellConfig?: {
    promptTemplate: string;
    aliasMapping: string;
  };
}

export interface GetShellConfigVariables {
  id: UUIDString;
}

export interface GetTerminalThemeData {
  terminalTheme?: {
    name: string;
    colorPalette: string;
  };
}

export interface GetTerminalThemeVariables {
  id: UUIDString;
}

export interface ListAllUsersData {
  users: ({
    username: string;
    avatarUrl?: string | null;
  })[];
}

export interface ListMyCommandHistoryData {
  commandHistories: ({
    commandString: string;
  })[];
}

export interface ListMyDevicesData {
  deviceSyncs: ({
    deviceName: string;
    lastSeen: TimestampString;
  })[];
}

export interface ListMyProfilesData {
  profiles: ({
    name: string;
    isDefault?: boolean | null;
  })[];
}

export interface ListMyShellConfigsData {
  shellConfigs: ({
    promptTemplate: string;
  })[];
}

export interface ListMyTerminalThemesData {
  terminalThemes: ({
    name: string;
  })[];
}

export interface Profile_Key {
  id: UUIDString;
  __typename?: 'Profile_Key';
}

export interface ShellConfig_Key {
  id: UUIDString;
  __typename?: 'ShellConfig_Key';
}

export interface SyncDeviceData {
  deviceSync_insert: DeviceSync_Key;
}

export interface SyncDeviceVariables {
  name: string;
}

export interface TerminalTheme_Key {
  id: UUIDString;
  __typename?: 'TerminalTheme_Key';
}

export interface UpdateDeviceSyncData {
  deviceSync_update?: DeviceSync_Key | null;
}

export interface UpdateDeviceSyncVariables {
  id: UUIDString;
  profileId?: UUIDString | null;
}

export interface UpdateProfileData {
  profile_update?: Profile_Key | null;
}

export interface UpdateProfileVariables {
  id: UUIDString;
  name?: string | null;
}

export interface UpdateShellConfigData {
  shellConfig_update?: ShellConfig_Key | null;
}

export interface UpdateShellConfigVariables {
  id: UUIDString;
  prompt?: string | null;
}

export interface UpdateTerminalThemeData {
  terminalTheme_update?: TerminalTheme_Key | null;
}

export interface UpdateTerminalThemeVariables {
  id: UUIDString;
  opacity?: number | null;
}

export interface UpdateUserData {
  user_update?: User_Key | null;
}

export interface UpdateUserVariables {
  username: string;
}

export interface User_Key {
  id: UUIDString;
  __typename?: 'User_Key';
}

interface CreateUserRef {
  /* Allow users to create refs without passing in DataConnect */
  (): MutationRef<CreateUserData, undefined>;
  /* Allow users to pass in custom DataConnect instances */
  (dc: DataConnect): MutationRef<CreateUserData, undefined>;
  operationName: string;
}
export const createUserRef: CreateUserRef;

export function createUser(): MutationPromise<CreateUserData, undefined>;
export function createUser(dc: DataConnect): MutationPromise<CreateUserData, undefined>;

interface UpdateUserRef {
  /* Allow users to create refs without passing in DataConnect */
  (vars: UpdateUserVariables): MutationRef<UpdateUserData, UpdateUserVariables>;
  /* Allow users to pass in custom DataConnect instances */
  (dc: DataConnect, vars: UpdateUserVariables): MutationRef<UpdateUserData, UpdateUserVariables>;
  operationName: string;
}
export const updateUserRef: UpdateUserRef;

export function updateUser(vars: UpdateUserVariables): MutationPromise<UpdateUserData, UpdateUserVariables>;
export function updateUser(dc: DataConnect, vars: UpdateUserVariables): MutationPromise<UpdateUserData, UpdateUserVariables>;

interface DeleteUserRef {
  /* Allow users to create refs without passing in DataConnect */
  (): MutationRef<DeleteUserData, undefined>;
  /* Allow users to pass in custom DataConnect instances */
  (dc: DataConnect): MutationRef<DeleteUserData, undefined>;
  operationName: string;
}
export const deleteUserRef: DeleteUserRef;

export function deleteUser(): MutationPromise<DeleteUserData, undefined>;
export function deleteUser(dc: DataConnect): MutationPromise<DeleteUserData, undefined>;

interface GetCurrentUserRef {
  /* Allow users to create refs without passing in DataConnect */
  (): QueryRef<GetCurrentUserData, undefined>;
  /* Allow users to pass in custom DataConnect instances */
  (dc: DataConnect): QueryRef<GetCurrentUserData, undefined>;
  operationName: string;
}
export const getCurrentUserRef: GetCurrentUserRef;

export function getCurrentUser(options?: ExecuteQueryOptions): QueryPromise<GetCurrentUserData, undefined>;
export function getCurrentUser(dc: DataConnect, options?: ExecuteQueryOptions): QueryPromise<GetCurrentUserData, undefined>;

interface ListAllUsersRef {
  /* Allow users to create refs without passing in DataConnect */
  (): QueryRef<ListAllUsersData, undefined>;
  /* Allow users to pass in custom DataConnect instances */
  (dc: DataConnect): QueryRef<ListAllUsersData, undefined>;
  operationName: string;
}
export const listAllUsersRef: ListAllUsersRef;

export function listAllUsers(options?: ExecuteQueryOptions): QueryPromise<ListAllUsersData, undefined>;
export function listAllUsers(dc: DataConnect, options?: ExecuteQueryOptions): QueryPromise<ListAllUsersData, undefined>;

interface CreateProfileRef {
  /* Allow users to create refs without passing in DataConnect */
  (vars: CreateProfileVariables): MutationRef<CreateProfileData, CreateProfileVariables>;
  /* Allow users to pass in custom DataConnect instances */
  (dc: DataConnect, vars: CreateProfileVariables): MutationRef<CreateProfileData, CreateProfileVariables>;
  operationName: string;
}
export const createProfileRef: CreateProfileRef;

export function createProfile(vars: CreateProfileVariables): MutationPromise<CreateProfileData, CreateProfileVariables>;
export function createProfile(dc: DataConnect, vars: CreateProfileVariables): MutationPromise<CreateProfileData, CreateProfileVariables>;

interface UpdateProfileRef {
  /* Allow users to create refs without passing in DataConnect */
  (vars: UpdateProfileVariables): MutationRef<UpdateProfileData, UpdateProfileVariables>;
  /* Allow users to pass in custom DataConnect instances */
  (dc: DataConnect, vars: UpdateProfileVariables): MutationRef<UpdateProfileData, UpdateProfileVariables>;
  operationName: string;
}
export const updateProfileRef: UpdateProfileRef;

export function updateProfile(vars: UpdateProfileVariables): MutationPromise<UpdateProfileData, UpdateProfileVariables>;
export function updateProfile(dc: DataConnect, vars: UpdateProfileVariables): MutationPromise<UpdateProfileData, UpdateProfileVariables>;

interface DeleteProfileRef {
  /* Allow users to create refs without passing in DataConnect */
  (vars: DeleteProfileVariables): MutationRef<DeleteProfileData, DeleteProfileVariables>;
  /* Allow users to pass in custom DataConnect instances */
  (dc: DataConnect, vars: DeleteProfileVariables): MutationRef<DeleteProfileData, DeleteProfileVariables>;
  operationName: string;
}
export const deleteProfileRef: DeleteProfileRef;

export function deleteProfile(vars: DeleteProfileVariables): MutationPromise<DeleteProfileData, DeleteProfileVariables>;
export function deleteProfile(dc: DataConnect, vars: DeleteProfileVariables): MutationPromise<DeleteProfileData, DeleteProfileVariables>;

interface GetProfileRef {
  /* Allow users to create refs without passing in DataConnect */
  (vars: GetProfileVariables): QueryRef<GetProfileData, GetProfileVariables>;
  /* Allow users to pass in custom DataConnect instances */
  (dc: DataConnect, vars: GetProfileVariables): QueryRef<GetProfileData, GetProfileVariables>;
  operationName: string;
}
export const getProfileRef: GetProfileRef;

export function getProfile(vars: GetProfileVariables, options?: ExecuteQueryOptions): QueryPromise<GetProfileData, GetProfileVariables>;
export function getProfile(dc: DataConnect, vars: GetProfileVariables, options?: ExecuteQueryOptions): QueryPromise<GetProfileData, GetProfileVariables>;

interface ListMyProfilesRef {
  /* Allow users to create refs without passing in DataConnect */
  (): QueryRef<ListMyProfilesData, undefined>;
  /* Allow users to pass in custom DataConnect instances */
  (dc: DataConnect): QueryRef<ListMyProfilesData, undefined>;
  operationName: string;
}
export const listMyProfilesRef: ListMyProfilesRef;

export function listMyProfiles(options?: ExecuteQueryOptions): QueryPromise<ListMyProfilesData, undefined>;
export function listMyProfiles(dc: DataConnect, options?: ExecuteQueryOptions): QueryPromise<ListMyProfilesData, undefined>;

interface CreateShellConfigRef {
  /* Allow users to create refs without passing in DataConnect */
  (vars: CreateShellConfigVariables): MutationRef<CreateShellConfigData, CreateShellConfigVariables>;
  /* Allow users to pass in custom DataConnect instances */
  (dc: DataConnect, vars: CreateShellConfigVariables): MutationRef<CreateShellConfigData, CreateShellConfigVariables>;
  operationName: string;
}
export const createShellConfigRef: CreateShellConfigRef;

export function createShellConfig(vars: CreateShellConfigVariables): MutationPromise<CreateShellConfigData, CreateShellConfigVariables>;
export function createShellConfig(dc: DataConnect, vars: CreateShellConfigVariables): MutationPromise<CreateShellConfigData, CreateShellConfigVariables>;

interface UpdateShellConfigRef {
  /* Allow users to create refs without passing in DataConnect */
  (vars: UpdateShellConfigVariables): MutationRef<UpdateShellConfigData, UpdateShellConfigVariables>;
  /* Allow users to pass in custom DataConnect instances */
  (dc: DataConnect, vars: UpdateShellConfigVariables): MutationRef<UpdateShellConfigData, UpdateShellConfigVariables>;
  operationName: string;
}
export const updateShellConfigRef: UpdateShellConfigRef;

export function updateShellConfig(vars: UpdateShellConfigVariables): MutationPromise<UpdateShellConfigData, UpdateShellConfigVariables>;
export function updateShellConfig(dc: DataConnect, vars: UpdateShellConfigVariables): MutationPromise<UpdateShellConfigData, UpdateShellConfigVariables>;

interface DeleteShellConfigRef {
  /* Allow users to create refs without passing in DataConnect */
  (vars: DeleteShellConfigVariables): MutationRef<DeleteShellConfigData, DeleteShellConfigVariables>;
  /* Allow users to pass in custom DataConnect instances */
  (dc: DataConnect, vars: DeleteShellConfigVariables): MutationRef<DeleteShellConfigData, DeleteShellConfigVariables>;
  operationName: string;
}
export const deleteShellConfigRef: DeleteShellConfigRef;

export function deleteShellConfig(vars: DeleteShellConfigVariables): MutationPromise<DeleteShellConfigData, DeleteShellConfigVariables>;
export function deleteShellConfig(dc: DataConnect, vars: DeleteShellConfigVariables): MutationPromise<DeleteShellConfigData, DeleteShellConfigVariables>;

interface GetShellConfigRef {
  /* Allow users to create refs without passing in DataConnect */
  (vars: GetShellConfigVariables): QueryRef<GetShellConfigData, GetShellConfigVariables>;
  /* Allow users to pass in custom DataConnect instances */
  (dc: DataConnect, vars: GetShellConfigVariables): QueryRef<GetShellConfigData, GetShellConfigVariables>;
  operationName: string;
}
export const getShellConfigRef: GetShellConfigRef;

export function getShellConfig(vars: GetShellConfigVariables, options?: ExecuteQueryOptions): QueryPromise<GetShellConfigData, GetShellConfigVariables>;
export function getShellConfig(dc: DataConnect, vars: GetShellConfigVariables, options?: ExecuteQueryOptions): QueryPromise<GetShellConfigData, GetShellConfigVariables>;

interface ListMyShellConfigsRef {
  /* Allow users to create refs without passing in DataConnect */
  (): QueryRef<ListMyShellConfigsData, undefined>;
  /* Allow users to pass in custom DataConnect instances */
  (dc: DataConnect): QueryRef<ListMyShellConfigsData, undefined>;
  operationName: string;
}
export const listMyShellConfigsRef: ListMyShellConfigsRef;

export function listMyShellConfigs(options?: ExecuteQueryOptions): QueryPromise<ListMyShellConfigsData, undefined>;
export function listMyShellConfigs(dc: DataConnect, options?: ExecuteQueryOptions): QueryPromise<ListMyShellConfigsData, undefined>;

interface AddCommandHistoryRef {
  /* Allow users to create refs without passing in DataConnect */
  (vars: AddCommandHistoryVariables): MutationRef<AddCommandHistoryData, AddCommandHistoryVariables>;
  /* Allow users to pass in custom DataConnect instances */
  (dc: DataConnect, vars: AddCommandHistoryVariables): MutationRef<AddCommandHistoryData, AddCommandHistoryVariables>;
  operationName: string;
}
export const addCommandHistoryRef: AddCommandHistoryRef;

export function addCommandHistory(vars: AddCommandHistoryVariables): MutationPromise<AddCommandHistoryData, AddCommandHistoryVariables>;
export function addCommandHistory(dc: DataConnect, vars: AddCommandHistoryVariables): MutationPromise<AddCommandHistoryData, AddCommandHistoryVariables>;

interface DeleteCommandHistoryRef {
  /* Allow users to create refs without passing in DataConnect */
  (vars: DeleteCommandHistoryVariables): MutationRef<DeleteCommandHistoryData, DeleteCommandHistoryVariables>;
  /* Allow users to pass in custom DataConnect instances */
  (dc: DataConnect, vars: DeleteCommandHistoryVariables): MutationRef<DeleteCommandHistoryData, DeleteCommandHistoryVariables>;
  operationName: string;
}
export const deleteCommandHistoryRef: DeleteCommandHistoryRef;

export function deleteCommandHistory(vars: DeleteCommandHistoryVariables): MutationPromise<DeleteCommandHistoryData, DeleteCommandHistoryVariables>;
export function deleteCommandHistory(dc: DataConnect, vars: DeleteCommandHistoryVariables): MutationPromise<DeleteCommandHistoryData, DeleteCommandHistoryVariables>;

interface GetCommandHistoryRef {
  /* Allow users to create refs without passing in DataConnect */
  (vars: GetCommandHistoryVariables): QueryRef<GetCommandHistoryData, GetCommandHistoryVariables>;
  /* Allow users to pass in custom DataConnect instances */
  (dc: DataConnect, vars: GetCommandHistoryVariables): QueryRef<GetCommandHistoryData, GetCommandHistoryVariables>;
  operationName: string;
}
export const getCommandHistoryRef: GetCommandHistoryRef;

export function getCommandHistory(vars: GetCommandHistoryVariables, options?: ExecuteQueryOptions): QueryPromise<GetCommandHistoryData, GetCommandHistoryVariables>;
export function getCommandHistory(dc: DataConnect, vars: GetCommandHistoryVariables, options?: ExecuteQueryOptions): QueryPromise<GetCommandHistoryData, GetCommandHistoryVariables>;

interface ListMyCommandHistoryRef {
  /* Allow users to create refs without passing in DataConnect */
  (): QueryRef<ListMyCommandHistoryData, undefined>;
  /* Allow users to pass in custom DataConnect instances */
  (dc: DataConnect): QueryRef<ListMyCommandHistoryData, undefined>;
  operationName: string;
}
export const listMyCommandHistoryRef: ListMyCommandHistoryRef;

export function listMyCommandHistory(options?: ExecuteQueryOptions): QueryPromise<ListMyCommandHistoryData, undefined>;
export function listMyCommandHistory(dc: DataConnect, options?: ExecuteQueryOptions): QueryPromise<ListMyCommandHistoryData, undefined>;

interface CreateTerminalThemeRef {
  /* Allow users to create refs without passing in DataConnect */
  (vars: CreateTerminalThemeVariables): MutationRef<CreateTerminalThemeData, CreateTerminalThemeVariables>;
  /* Allow users to pass in custom DataConnect instances */
  (dc: DataConnect, vars: CreateTerminalThemeVariables): MutationRef<CreateTerminalThemeData, CreateTerminalThemeVariables>;
  operationName: string;
}
export const createTerminalThemeRef: CreateTerminalThemeRef;

export function createTerminalTheme(vars: CreateTerminalThemeVariables): MutationPromise<CreateTerminalThemeData, CreateTerminalThemeVariables>;
export function createTerminalTheme(dc: DataConnect, vars: CreateTerminalThemeVariables): MutationPromise<CreateTerminalThemeData, CreateTerminalThemeVariables>;

interface UpdateTerminalThemeRef {
  /* Allow users to create refs without passing in DataConnect */
  (vars: UpdateTerminalThemeVariables): MutationRef<UpdateTerminalThemeData, UpdateTerminalThemeVariables>;
  /* Allow users to pass in custom DataConnect instances */
  (dc: DataConnect, vars: UpdateTerminalThemeVariables): MutationRef<UpdateTerminalThemeData, UpdateTerminalThemeVariables>;
  operationName: string;
}
export const updateTerminalThemeRef: UpdateTerminalThemeRef;

export function updateTerminalTheme(vars: UpdateTerminalThemeVariables): MutationPromise<UpdateTerminalThemeData, UpdateTerminalThemeVariables>;
export function updateTerminalTheme(dc: DataConnect, vars: UpdateTerminalThemeVariables): MutationPromise<UpdateTerminalThemeData, UpdateTerminalThemeVariables>;

interface DeleteTerminalThemeRef {
  /* Allow users to create refs without passing in DataConnect */
  (vars: DeleteTerminalThemeVariables): MutationRef<DeleteTerminalThemeData, DeleteTerminalThemeVariables>;
  /* Allow users to pass in custom DataConnect instances */
  (dc: DataConnect, vars: DeleteTerminalThemeVariables): MutationRef<DeleteTerminalThemeData, DeleteTerminalThemeVariables>;
  operationName: string;
}
export const deleteTerminalThemeRef: DeleteTerminalThemeRef;

export function deleteTerminalTheme(vars: DeleteTerminalThemeVariables): MutationPromise<DeleteTerminalThemeData, DeleteTerminalThemeVariables>;
export function deleteTerminalTheme(dc: DataConnect, vars: DeleteTerminalThemeVariables): MutationPromise<DeleteTerminalThemeData, DeleteTerminalThemeVariables>;

interface GetTerminalThemeRef {
  /* Allow users to create refs without passing in DataConnect */
  (vars: GetTerminalThemeVariables): QueryRef<GetTerminalThemeData, GetTerminalThemeVariables>;
  /* Allow users to pass in custom DataConnect instances */
  (dc: DataConnect, vars: GetTerminalThemeVariables): QueryRef<GetTerminalThemeData, GetTerminalThemeVariables>;
  operationName: string;
}
export const getTerminalThemeRef: GetTerminalThemeRef;

export function getTerminalTheme(vars: GetTerminalThemeVariables, options?: ExecuteQueryOptions): QueryPromise<GetTerminalThemeData, GetTerminalThemeVariables>;
export function getTerminalTheme(dc: DataConnect, vars: GetTerminalThemeVariables, options?: ExecuteQueryOptions): QueryPromise<GetTerminalThemeData, GetTerminalThemeVariables>;

interface ListMyTerminalThemesRef {
  /* Allow users to create refs without passing in DataConnect */
  (): QueryRef<ListMyTerminalThemesData, undefined>;
  /* Allow users to pass in custom DataConnect instances */
  (dc: DataConnect): QueryRef<ListMyTerminalThemesData, undefined>;
  operationName: string;
}
export const listMyTerminalThemesRef: ListMyTerminalThemesRef;

export function listMyTerminalThemes(options?: ExecuteQueryOptions): QueryPromise<ListMyTerminalThemesData, undefined>;
export function listMyTerminalThemes(dc: DataConnect, options?: ExecuteQueryOptions): QueryPromise<ListMyTerminalThemesData, undefined>;

interface SyncDeviceRef {
  /* Allow users to create refs without passing in DataConnect */
  (vars: SyncDeviceVariables): MutationRef<SyncDeviceData, SyncDeviceVariables>;
  /* Allow users to pass in custom DataConnect instances */
  (dc: DataConnect, vars: SyncDeviceVariables): MutationRef<SyncDeviceData, SyncDeviceVariables>;
  operationName: string;
}
export const syncDeviceRef: SyncDeviceRef;

export function syncDevice(vars: SyncDeviceVariables): MutationPromise<SyncDeviceData, SyncDeviceVariables>;
export function syncDevice(dc: DataConnect, vars: SyncDeviceVariables): MutationPromise<SyncDeviceData, SyncDeviceVariables>;

interface UpdateDeviceSyncRef {
  /* Allow users to create refs without passing in DataConnect */
  (vars: UpdateDeviceSyncVariables): MutationRef<UpdateDeviceSyncData, UpdateDeviceSyncVariables>;
  /* Allow users to pass in custom DataConnect instances */
  (dc: DataConnect, vars: UpdateDeviceSyncVariables): MutationRef<UpdateDeviceSyncData, UpdateDeviceSyncVariables>;
  operationName: string;
}
export const updateDeviceSyncRef: UpdateDeviceSyncRef;

export function updateDeviceSync(vars: UpdateDeviceSyncVariables): MutationPromise<UpdateDeviceSyncData, UpdateDeviceSyncVariables>;
export function updateDeviceSync(dc: DataConnect, vars: UpdateDeviceSyncVariables): MutationPromise<UpdateDeviceSyncData, UpdateDeviceSyncVariables>;

interface DeleteDeviceSyncRef {
  /* Allow users to create refs without passing in DataConnect */
  (vars: DeleteDeviceSyncVariables): MutationRef<DeleteDeviceSyncData, DeleteDeviceSyncVariables>;
  /* Allow users to pass in custom DataConnect instances */
  (dc: DataConnect, vars: DeleteDeviceSyncVariables): MutationRef<DeleteDeviceSyncData, DeleteDeviceSyncVariables>;
  operationName: string;
}
export const deleteDeviceSyncRef: DeleteDeviceSyncRef;

export function deleteDeviceSync(vars: DeleteDeviceSyncVariables): MutationPromise<DeleteDeviceSyncData, DeleteDeviceSyncVariables>;
export function deleteDeviceSync(dc: DataConnect, vars: DeleteDeviceSyncVariables): MutationPromise<DeleteDeviceSyncData, DeleteDeviceSyncVariables>;

interface GetDeviceSyncRef {
  /* Allow users to create refs without passing in DataConnect */
  (vars: GetDeviceSyncVariables): QueryRef<GetDeviceSyncData, GetDeviceSyncVariables>;
  /* Allow users to pass in custom DataConnect instances */
  (dc: DataConnect, vars: GetDeviceSyncVariables): QueryRef<GetDeviceSyncData, GetDeviceSyncVariables>;
  operationName: string;
}
export const getDeviceSyncRef: GetDeviceSyncRef;

export function getDeviceSync(vars: GetDeviceSyncVariables, options?: ExecuteQueryOptions): QueryPromise<GetDeviceSyncData, GetDeviceSyncVariables>;
export function getDeviceSync(dc: DataConnect, vars: GetDeviceSyncVariables, options?: ExecuteQueryOptions): QueryPromise<GetDeviceSyncData, GetDeviceSyncVariables>;

interface ListMyDevicesRef {
  /* Allow users to create refs without passing in DataConnect */
  (): QueryRef<ListMyDevicesData, undefined>;
  /* Allow users to pass in custom DataConnect instances */
  (dc: DataConnect): QueryRef<ListMyDevicesData, undefined>;
  operationName: string;
}
export const listMyDevicesRef: ListMyDevicesRef;

export function listMyDevices(options?: ExecuteQueryOptions): QueryPromise<ListMyDevicesData, undefined>;
export function listMyDevices(dc: DataConnect, options?: ExecuteQueryOptions): QueryPromise<ListMyDevicesData, undefined>;

