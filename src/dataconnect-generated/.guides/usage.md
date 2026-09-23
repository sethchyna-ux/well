# Basic Usage

Always prioritize using a supported framework over using the generated SDK
directly. Supported frameworks simplify the developer experience and help ensure
best practices are followed.





## Advanced Usage
If a user is not using a supported framework, they can use the generated SDK directly.

Here's an example of how to use it with the first 5 operations:

```js
import { createUser, updateUser, deleteUser, getCurrentUser, listAllUsers, createProfile, updateProfile, deleteProfile, getProfile, listMyProfiles } from '@dataconnect/generated';


// Operation CreateUser: 
const { data } = await CreateUser(dataConnect);

// Operation UpdateUser:  For variables, look at type UpdateUserVars in ../index.d.ts
const { data } = await UpdateUser(dataConnect, updateUserVars);

// Operation DeleteUser: 
const { data } = await DeleteUser(dataConnect);

// Operation GetCurrentUser: 
const { data } = await GetCurrentUser(dataConnect);

// Operation ListAllUsers: 
const { data } = await ListAllUsers(dataConnect);

// Operation CreateProfile:  For variables, look at type CreateProfileVars in ../index.d.ts
const { data } = await CreateProfile(dataConnect, createProfileVars);

// Operation UpdateProfile:  For variables, look at type UpdateProfileVars in ../index.d.ts
const { data } = await UpdateProfile(dataConnect, updateProfileVars);

// Operation DeleteProfile:  For variables, look at type DeleteProfileVars in ../index.d.ts
const { data } = await DeleteProfile(dataConnect, deleteProfileVars);

// Operation GetProfile:  For variables, look at type GetProfileVars in ../index.d.ts
const { data } = await GetProfile(dataConnect, getProfileVars);

// Operation ListMyProfiles: 
const { data } = await ListMyProfiles(dataConnect);


```